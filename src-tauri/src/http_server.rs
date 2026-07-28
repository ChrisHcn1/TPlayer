use std::fs::File;
use std::io::{prelude::*, Seek, SeekFrom};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use once_cell::sync::Lazy;
use urlencoding;

// 日志开关：设置为 true 可启用详细诊断日志
const ENABLE_LOGS: bool = true;

// MSIX 环境诊断标志
static MSIX_DIAGNOSTIC_DONE: once_cell::sync::Lazy<Mutex<bool>> = once_cell::sync::Lazy::new(|| Mutex::new(false));

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

// MSIX 环境诊断函数
fn run_msix_diagnostic_impl() {
    log_info!("========== MSIX 环境诊断 ==========");
    
    // 检查是否在 MSIX 环境下运行
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "未知".to_string());
    
    log_info!("进程路径: {}", exe_path);
    
    // MSIX 环境特征检测
    let is_msix = exe_path.contains("WindowsApps") || exe_path.contains("Program Files");
    log_info!("MSIX 环境检测: {}", if is_msix { "是" } else { "否" });
    
    // 检查网络相关环境变量
    log_info!("网络环境检查:");
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        log_info!("  LOCALAPPDATA: {}", local_app_data);
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        log_info!("  USERPROFILE: {}", user_profile);
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        log_info!("  APPDATA: {}", app_data);
    }
    
    // 检查网络接口
    log_info!("网络接口检查:");
    if let Ok(interfaces) = std::net::TcpListener::bind("127.0.0.1:0") {
        log_info!("  ✓ 可以创建 TCP socket");
        // 立即释放测试 socket
        let _ = interfaces;
    } else {
        log_error!("  ✗ 无法创建 TCP socket - 网络权限可能受限");
    }
    
    // 检查文件系统访问权限
    log_info!("文件系统访问检查:");
    let test_path = std::path::Path::new("C:\\Users");
    if test_path.exists() {
        log_info!("  ✓ 可以访问 C:\\Users");
    } else {
        log_error!("  ✗ 无法访问 C:\\Users - 文件系统权限受限");
    }
    
    log_info!("========== MSIX 环境诊断结束 ==========");
}

impl HttpServer {
    // MSIX 环境诊断
    fn run_msix_diagnostic() {
        run_msix_diagnostic_impl();
    }
    // 创建并启动 HTTP 服务器
    pub fn start() -> Result<Self, String> {
        // MSIX 环境诊断
        {
            let mut diagnostic_done = MSIX_DIAGNOSTIC_DONE.lock().unwrap();
            if !*diagnostic_done {
                *diagnostic_done = true;
                Self::run_msix_diagnostic();
            }
        }
        
        log_info!("========== HTTP服务器启动诊断开始 ==========");
        log_info!("当前进程路径: {:?}", std::env::current_exe());
        log_info!("当前工作目录: {:?}", std::env::current_dir());
        log_info!("应用数据目录: {:?}", std::env::var("LOCALAPPDATA").ok());
        log_info!("用户目录: {:?}", std::env::var("USERPROFILE").ok());
        
        // 尝试在 8000-9000 端口范围内找到一个可用端口
        log_info!("开始扫描可用端口 (8000-9000)...");
        let port = (8000..=9000)
            .find(|&p| {
                let result = TcpListener::bind("127.0.0.1:".to_string() + &p.to_string());
                log_info!("尝试绑定端口 {}: {}", p, if result.is_ok() { "成功" } else { "失败" });
                result.is_ok()
            })
            .ok_or_else(|| {
                let error_msg = "无法找到可用端口，端口范围 8000-9000 全部被占用或网络权限受限".to_string();
                log_error!("❌ {}", error_msg);
                log_error!("可能原因: MSIX 沙箱限制网络访问，或防火墙阻止端口绑定");
                error_msg
            })?;

        log_info!("✅ 找到可用端口: {}", port);
        
        let listener = TcpListener::bind("127.0.0.1:".to_string() + &port.to_string())
            .map_err(|e| {
                let error_msg = format!("绑定端口 {} 失败: {}", port, e);
                log_error!("❌ {}", error_msg);
                error_msg
            })?;

        log_info!("✅ HTTP服务器成功绑定端口: {}", port);

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
        // 空 token 直接拒绝，避免空值误匹配
        if valid_token.is_empty() {
            return false;
        }

        // 从请求行提取路径（含查询字符串）
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        // 提取查询字符串部分
        let query = match path.split('?').nth(1) {
            Some(q) => q,
            None => return false,
        };

        // 精确匹配 token 查询参数，避免子串误判（如 "xtoken=" 或 "token=abcde" 误命中）
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some("token") {
                if let Some(value) = kv.next() {
                    if value == valid_token {
                        return true;
                    }
                }
            }
        }
        false
    }

    // 处理HTTP连接
    fn handle_connection(mut stream: TcpStream, token: &str) {
        let mut buffer = [0; 8192];
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
                        // 解析 Range 头 (格式: "bytes=start-end"，支持 "start-"、"start-end"、"-suffix")
                        log_info!("收到 Range 请求: {}", range);
                        let range = range.strip_prefix("bytes=").unwrap_or(range);
                        let parts: Vec<&str> = range.splitn(2, '-').collect();

                        // 解析失败时回退到完整文件响应（200），避免 unwrap_or 静默掩盖错误
                        let parsed_range = if parts.len() == 2 {
                            let start_str = parts[0].trim();
                            let end_str = parts[1].trim();

                            let start = if start_str.is_empty() {
                                None
                            } else {
                                match start_str.parse::<u64>() {
                                    Ok(v) => Some(v),
                                    Err(_) => {
                                        log_error!("Range 起始位置解析失败: '{}'", start_str);
                                        None
                                    }
                                }
                            };

                            let end = if end_str.is_empty() {
                                None
                            } else {
                                match end_str.parse::<u64>() {
                                    Ok(v) => Some(v),
                                    Err(_) => {
                                        log_error!("Range 结束位置解析失败: '{}'", end_str);
                                        None
                                    }
                                }
                            };

                            match (start, end) {
                                (Some(s), Some(e)) => Some((s, e)),
                                (Some(s), None) => Some((s, file_size.saturating_sub(1))),
                                (None, Some(e)) => {
                                    // 后缀范围：请求最后 e 个字节
                                    if e == 0 || file_size == 0 {
                                        None
                                    } else {
                                        Some((file_size.saturating_sub(e), file_size.saturating_sub(1)))
                                    }
                                }
                                (None, None) => None,
                            }
                        } else {
                            None
                        };

                        // 应用边界约束并生成响应
                        match parsed_range {
                            Some((start, end)) if file_size > 0 => {
                                let start = start.min(file_size);
                                let end = end.min(file_size.saturating_sub(1));
                                if start > end {
                                    (0, None, 200, None)
                                } else {
                                    (start, Some(end), 206, Some(format!("bytes {}-{}/{}", start, end, file_size)))
                                }
                            }
                            _ => (0, None, 200, None),
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
                        // content_range 为 None 时不能输出空的 Content-Range 头，
                        // 否则会产生非法的 HTTP 响应，此时退回到 200 完整响应
                        match content_range {
                            Some(cr) => format!(
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
                                cr
                            ),
                            None => {
                                log_error!("206 响应缺少 Content-Range，退回 200 响应");
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
                            }
                        }
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
                    if let Err(e) = stream.flush() {
                        log_error!("刷新响应头失败: {}", e);
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
                    
                    if let Err(e) = stream.flush() {
                        log_error!("刷新文件内容失败: {}", e);
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
            "audio/x-wav"
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
        let _ = stream.flush();
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

// 检查HTTP服务器状态（用于诊断）
#[tauri::command]
pub fn check_http_server_status() -> Result<serde_json::Value, String> {
    let server = get_http_server();
    
    if let Some(s) = server {
        Ok(serde_json::json!({
            "status": "running",
            "port": s.port,
            "token": s.token,
            "url": s.get_url()
        }))
    } else {
        Ok(serde_json::json!({
            "status": "not_running",
            "error": "HTTP服务器未启动"
        }))
    }
}

// 打开开发者工具
#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) -> Result<(), String> {
    log_info!("尝试打开开发者工具");
    window.open_devtools();
    Ok(())
}
