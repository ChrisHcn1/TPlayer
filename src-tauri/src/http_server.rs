use std::fs::File;
use std::io::{prelude::*, Seek, SeekFrom};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;
use urlencoding;

// 日志开关：设置为 true 可启用日志输出
const ENABLE_LOGS: bool = true;

// 条件性日志宏
macro_rules! log_info {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            println!($($arg)*);
        }
    };
}

macro_rules! log_error {
    ($($arg:tt)*) => {
        if ENABLE_LOGS {
            eprintln!($($arg)*);
        }
    };
}

// 全局Token存储，用于鉴权
static SERVER_TOKEN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

// 生成随机Token
fn generate_token() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", timestamp)
}

// 设置服务器Token
pub fn set_server_token(token: &str) {
    *SERVER_TOKEN.lock().unwrap() = token.to_string();
}

// 获取服务器Token
pub fn get_server_token() -> String {
    SERVER_TOKEN.lock().unwrap().clone()
}

// HTTP服务器
#[derive(Clone)]
pub struct HttpServer {
    listener: Arc<Option<TcpListener>>,
    port: u16,
    token: String,
}

impl HttpServer {
    // 创建并启动 HTTP 服务器
    pub fn start() -> Result<Self, String> {
        // 尝试在 8000-9000 端口范围内找到一个可用端口
        let port = (8000..=9000)
            .find(|&p| TcpListener::bind("127.0.0.1:".to_string() + &p.to_string()).is_ok())
            .ok_or_else(|| "无法找到可用端口，端口范围 8000-9000 全部被占用".to_string())?;

        let listener = TcpListener::bind("127.0.0.1:".to_string() + &port.to_string())
            .map_err(|e| format!("绑定端口失败: {}", e))?;

        // 生成随机Token
        let token = generate_token();
        set_server_token(&token);
        
        log_info!("HTTP服务器启动在端口: {}", port);
        log_info!("HTTP服务器Token: {}", token);

        let server = Self {
            listener: Arc::new(Some(listener)),
            port,
            token,
        };

        // 启动服务器线程
        let server_clone = server.clone();
        thread::spawn(move || {
            server_clone.run();
        });

        Ok(server)
    }

    // 运行HTTP服务器
    fn run(&self) {
        if let Some(listener) = &*self.listener {
            log_info!("HTTP服务器开始监听端口: {}", self.port);
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        log_info!("收到新连接");
                        let token_clone = self.token.clone();
                        thread::spawn(move || {
                            Self::handle_connection(stream, &token_clone);
                        });
                    }
                    Err(e) => {
                        log_error!("接受连接失败: {}", e);
                    }
                }
            }
        }
    }

    // 验证请求中的Token
    fn validate_token(request: &str, valid_token: &str) -> bool {
        // 从URL参数中提取token
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        // 检查URL中是否包含有效的token参数
        let token_param = format!("token={}", valid_token);
        path.contains(&token_param)
    }

    // 处理HTTP连接
    fn handle_connection(mut stream: TcpStream, token: &str) {
        let mut buffer = [0; 1024];
        let bytes_read = match stream.read(&mut buffer) {
            Ok(n) => n,
            Err(e) => {
                log_error!("读取请求失败: {}", e);
                return;
            }
        };
        
        // 检查请求是否为空
        if bytes_read == 0 || buffer.iter().all(|&b| b == 0) {
            log_error!("收到空请求");
            return;
        }

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        log_info!("收到请求: {}", request.lines().next().unwrap_or(""));

        // 解析请求路径
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        // 处理文件请求（需要Token鉴权）
        if path.starts_with("/file/") {
            // 验证Token
            if !Self::validate_token(&request, token) {
                log_error!("请求未携带有效Token，拒绝访问");
                Self::send_error(&mut stream, 403, "Forbidden", "Invalid or missing token");
                return;
            }

            // 提取文件路径（移除/file/前缀和token参数）
            let mut encoded_path = &path[6..];
            
            // 移除可能存在的token参数
            if let Some(token_pos) = encoded_path.find("?token=") {
                encoded_path = &encoded_path[..token_pos];
            }

            // 解码URL编码的路径
            let decoded = match urlencoding::decode(encoded_path) {
                Ok(p) => p,
                Err(e) => {
                    log_error!("解码路径失败: {}", e);
                    Self::send_error(&mut stream, 400, "Bad Request", "Invalid URL encoding");
                    return;
                }
            };
            let file_path = decoded.as_ref();

            log_info!("请求文件: {}", file_path);

            // 读取文件
            match File::open(file_path) {
                Ok(mut file) => {
                    // 获取文件大小
                    let file_size = match file.metadata() {
                        Ok(m) => m.len(),
                        Err(e) => {
                            log_error!("获取文件元数据失败: {}", e);
                            Self::send_error(&mut stream, 500, "Internal Server Error", "Failed to get file metadata");
                            return;
                        }
                    };
                    
                    // 解析 Range 请求头
                    let range_header = request
                        .lines()
                        .find(|line| line.to_lowercase().starts_with("range:"))
                        .and_then(|line| line.split(':').nth(1).map(|s| s.trim()));

                    let (start_byte, end_byte, status_code, content_range) = if let Some(range) = range_header {
                        // 解析 Range 头 (格式: "bytes=start-end")
                        log_info!("收到 Range 请求: {}", range);
                        let range = range.strip_prefix("bytes=").unwrap_or(range);
                        let parts: Vec<&str> = range.split('-').collect();

                        if parts.len() >= 2 {
                            let start = parts[0].parse::<u64>().unwrap_or(0).min(file_size);
                            let end = if parts[1].is_empty() {
                                file_size.saturating_sub(1)
                            } else {
                                parts[1].parse::<u64>().unwrap_or(file_size.saturating_sub(1)).min(file_size.saturating_sub(1))
                            };
                            
                            // 检查边界条件
                            if start > end {
                                (0, None, 200, None)
                            } else {
                                (start, Some(end), 206, Some(format!("bytes {}-{}/{}", start, end, file_size)))
                            }
                        } else {
                            (0, None, 200, None)
                        }
                    } else {
                        (0, None, 200, None)
                    };

                    // 确定MIME类型
                    let mime_type = Self::get_mime_type(file_path);

                    // 发送HTTP响应头
                    let response = if status_code == 206 {
                        // 支持 Range 请求
                        let content_length = if let Some(end) = end_byte {
                            (end - start_byte + 1) as usize
                        } else {
                            (file_size - start_byte) as usize
                        };
                        format!(
                            "HTTP/1.1 206 Partial Content\r\n\
                             Content-Type: {}\r\n\
                             Content-Length: {}\r\n\
                             Content-Range: {}\r\n\
                             Accept-Ranges: bytes\r\n\
                             Connection: close\r\n\
                             Access-Control-Allow-Origin: *\r\n\
                             \r\n",
                            mime_type,
                            content_length,
                            content_range.unwrap_or_default()
                        )
                    } else {
                        // 普通 200 响应
                        format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: {}\r\n\
                             Content-Length: {}\r\n\
                             Accept-Ranges: bytes\r\n\
                             Connection: close\r\n\
                             Access-Control-Allow-Origin: *\r\n\
                             \r\n",
                            mime_type,
                            file_size
                        )
                    };
                    
                    if let Err(e) = stream.write(response.as_bytes()) {
                        log_error!("发送响应头失败: {}", e);
                        return;
                    }

                    // 如果是 Range 请求,先定位到起始位置
                    if start_byte > 0 {
                        if let Err(e) = file.seek(SeekFrom::Start(start_byte)) {
                            log_error!("文件定位失败: {}", e);
                            return;
                        }
                    }

                    // 发送文件内容（流式传输）
                    let mut buffer = [0; 8192];
                    if let Some(end) = end_byte {
                        // Range 请求,发送指定范围
                        let mut bytes_sent = start_byte;
                        while bytes_sent <= end {
                            let remaining = (end - bytes_sent + 1) as usize;
                            let to_read = buffer.len().min(remaining);
                            match file.read(&mut buffer[..to_read]) {
                                Ok(n) => {
                                    if n == 0 {
                                        break;
                                    }
                                    if let Err(e) = stream.write(&buffer[0..n]) {
                                        log_error!("发送文件失败: {}", e);
                                        break;
                                    }
                                    bytes_sent += n as u64;
                                }
                                Err(e) => {
                                    log_error!("读取文件失败: {}", e);
                                    break;
                                }
                            }
                        }
                    } else {
                        // 发送整个文件
                        while let Ok(n) = file.read(&mut buffer) {
                            if n == 0 {
                                break;
                            }
                            if let Err(e) = stream.write(&buffer[0..n]) {
                                log_error!("发送文件失败: {}", e);
                                break;
                            }
                        }
                    }
                    
                    log_info!("文件发送完成: {}", file_path);
                }
                Err(e) => {
                    log_error!("打开文件失败: {} - 路径: {}", e, file_path);
                    let body = format!("File not found: {}", e);
                    Self::send_error(&mut stream, 404, "Not Found", &body);
                }
            }
        } else {
            // 处理其他请求
            let response = "HTTP/1.1 404 Not Found\r\n"
                .to_string() + "Content-Type: text/plain\r\n"
                + "Content-Length: 9\r\n"
                + "Connection: close\r\n"
                + "Access-Control-Allow-Origin: *\r\n"
                + "\r\n"
                + "Not Found";
            let _ = stream.write(response.as_bytes());
        }
    }

    // 获取MIME类型
    fn get_mime_type(file_path: &str) -> &'static str {
        if file_path.ends_with(".mp3") {
            "audio/mpeg"
        } else if file_path.ends_with(".flac") {
            "audio/flac"
        } else if file_path.ends_with(".wav") {
            "audio/wav"
        } else if file_path.ends_with(".ogg") {
            "audio/ogg"
        } else if file_path.ends_with(".aac") {
            "audio/aac"
        } else if file_path.ends_with(".m4a") {
            "audio/mp4"
        } else if file_path.ends_with(".alac") {
            "audio/x-alac"
        } else if file_path.ends_with(".webm") {
            "audio/webm"
        } else if file_path.ends_with(".opus") {
            "audio/opus"
        } else if file_path.ends_with(".mid") || file_path.ends_with(".midi") {
            "audio/midi"
        } else if file_path.ends_with(".ac3") {
            "audio/ac3"
        } else if file_path.ends_with(".dts") {
            "audio/vnd.dts"
        } else if file_path.ends_with(".wma") {
            "audio/x-ms-wma"
        } else if file_path.ends_with(".ape") {
            "audio/ape"
        } else {
            // 默认音频类型
            "audio/mpeg"
        }
    }

    // 获取服务器URL
    pub fn get_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    // 停止服务器
    pub fn stop(&mut self) {
        // 由于使用了Arc，我们无法直接take，只能通过drop来释放
        if self.listener.is_some() {
            log_info!("HTTP服务器已停止");
        }
    }
    
    // 发送HTTP错误响应
    fn send_error(stream: &mut TcpStream, code: u16, status: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             Access-Control-Allow-Origin: *\r\n\
             \r\n\
             {}",
            code,
            status,
            body.len(),
            body
        );
        let _ = stream.write(response.as_bytes());
    }
}

// 全局HTTP服务器实例

static HTTP_SERVER: Lazy<Arc<Mutex<Option<HttpServer>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(None)));

// 初始化HTTP服务器
pub fn init_http_server() -> Result<(), String> {
    let server = HttpServer::start()?;
    let mut global = HTTP_SERVER.lock().unwrap();
    *global = Some(server);
    Ok(())
}

// 获取HTTP服务器
pub fn get_http_server() -> Option<Arc<HttpServer>> {
    let global = HTTP_SERVER.lock().unwrap();
    global.as_ref().map(|server| Arc::new(server.clone()))
}

// 获取文件的HTTP URL
pub fn get_file_url(file_path: &str) -> Option<String> {
    let server = get_http_server()?;
    let encoded_path = urlencoding::encode(file_path);
    // 获取服务器Token并添加到URL参数中
    let token = get_server_token();
    Some(format!("{}/file/{}?token={}", server.get_url(), encoded_path, token))
}

// 获取文件的HTTP URL（Tauri命令）
#[tauri::command]
pub fn get_file_http_url(file_path: String) -> Result<String, String> {
    get_file_url(&file_path).ok_or_else(|| "HTTP服务器未初始化".to_string())
}
