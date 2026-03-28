use std::fs::File;
use std::io::{prelude::*, Seek, SeekFrom};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
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

// HTTP服务器
#[derive(Clone)]
pub struct HttpServer {
    listener: Arc<Option<TcpListener>>,
    port: u16,
}

impl HttpServer {
    // 创建并启动HTTP服务器
    pub fn start() -> Result<Self, String> {
        // 尝试在8080-8100端口范围内找到一个可用端口
        let port = (8080..=8100)
            .find(|&p| TcpListener::bind("127.0.0.1:".to_string() + &p.to_string()).is_ok())
            .ok_or_else(|| "无法找到可用端口".to_string())?;

        let listener = TcpListener::bind("127.0.0.1:".to_string() + &port.to_string())
            .map_err(|e| format!("绑定端口失败: {}", e))?;

        log_info!("HTTP服务器启动在端口: {}", port);

        let server = Self {
            listener: Arc::new(Some(listener)),
            port,
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
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        thread::spawn(|| {
                            Self::handle_connection(stream);
                        });
                    }
                    Err(e) => {
                        log_error!("接受连接失败: {}", e);
                    }
                }
            }
        }
    }

    // 处理HTTP连接
    fn handle_connection(mut stream: TcpStream) {
        let mut buffer = [0; 1024];
        let _ = stream.read(&mut buffer);

        let request = String::from_utf8_lossy(&buffer[..]);
        log_info!("收到请求: {}", request.lines().next().unwrap_or(""));

        // 解析请求路径
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        // 处理文件请求
        if path.starts_with("/file/") {
            // 提取文件路径（移除/file/前缀）
            let file_path = &path[6..];
            // 解码URL编码的路径
            let file_path = urlencoding::decode(file_path)
                .map_err(|e| format!("解码路径失败: {}", e))
                .unwrap();

            log_info!("请求文件: {}", file_path);

            // 读取文件
            match File::open(&*file_path) {
                Ok(mut file) => {
                    // 获取文件大小
                    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
                    
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
                                file_size - 1
                            } else {
                                parts[1].parse::<u64>().unwrap_or(file_size - 1).min(file_size - 1)
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
                    let mime_type = if file_path.ends_with(".mp3") {
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
                    } else {
                        "application/octet-stream"
                    };

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
                             \r\n",
                            mime_type,
                            file_size
                        )
                    };
                    let _ = stream.write(response.as_bytes());

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
                }
                Err(e) => {
                    log_error!("打开文件失败: {}", e);
                    let response = "HTTP/1.1 404 Not Found\r\n"
                        .to_string() + "Content-Type: text/plain\r\n"
                        + "Content-Length: 9\r\n"
                        + "Connection: close\r\n"
                        + "\r\n"
                        + "Not Found";
                    let _ = stream.write(response.as_bytes());
                }
            }
        } else {
            // 处理其他请求
            let response = "HTTP/1.1 404 Not Found\r\n"
                .to_string() + "Content-Type: text/plain\r\n"
                + "Content-Length: 9\r\n"
                + "Connection: close\r\n"
                + "\r\n"
                + "Not Found";
            let _ = stream.write(response.as_bytes());
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
    Some(format!("{}/file/{}", server.get_url(), encoded_path))
}

// 获取文件的HTTP URL（Tauri命令）
#[tauri::command]
pub fn get_file_http_url(file_path: String) -> Result<String, String> {
    get_file_url(&file_path).ok_or_else(|| "HTTP服务器未初始化".to_string())
}
