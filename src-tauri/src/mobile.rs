use std::{
    fs, io,
    path::Path,
    net::{TcpListener, TcpStream},
    io::prelude::*,
};
use tauri::Manager;

#[cfg(target_os = "android")]
use std::os::unix::fs::PermissionsExt;

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        fs::create_dir_all(destination)?;
    }

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let src_path = entry.path();
        let dest_path = destination.join(entry.file_name());

        if metadata.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

fn start_local_embedded_server(_app: &tauri::App<tauri::Wry>) -> io::Result<()> {
    println!("Starting local embedded server for P2P architecture");

    // Start local HTTP server serving Rails content
    std::thread::spawn(|| {
        use std::net::TcpListener;
        use std::io::prelude::*;

        println!("Starting local embedded server on localhost:3000");

        let listener = TcpListener::bind("127.0.0.1:3000").expect("Failed to bind to port 3000");
        println!("Successfully bound to 127.0.0.1:3000");

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    std::thread::spawn(move || {
                        let mut buffer = [0; 1024];
                        if let Ok(size) = stream.read(&mut buffer) {
                            let request = String::from_utf8_lossy(&buffer[..size]);
                            println!("Received local request: {}", request.lines().next().unwrap_or(""));
                            let response = serve_cipher_interface(&request);
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                            println!("Sent local response");
                        }
                    });
                }
                Err(_) => {
                    // Handle error
                }
            }
        }
    });

    // Give server time to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    Ok(())
}

fn handle_request(mut stream: TcpStream) {
    let mut buffer = [0; 1024];

    if let Ok(size) = stream.read(&mut buffer) {
        let request = String::from_utf8_lossy(&buffer[..size]);
        let request_line = request.lines().next().unwrap_or("");
        println!("Received request: {}", request_line);

        // Serve Cipher interface response
        let response = serve_cipher_interface(&request);

        if let Err(e) = stream.write_all(response.as_bytes()) {
            println!("Failed to write response: {}", e);
        }

        if let Err(e) = stream.flush() {
            println!("Failed to flush stream: {}", e);
        }
    }
}

fn serve_cipher_interface(request: &str) -> String {
    // Parse the HTTP request to extract path
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return error_response("Empty request");
    }

    let request_line = lines[0];
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return error_response("Malformed request line");
    }

    let _method = parts[0];
    let path = parts[1];

    // Serve different responses based on path
    match path {
        "/" => cipher_home_response(),
        "/users/new" => signup_form_response(),
        "/users/sign_up" => signup_form_response(),
        "/users/sign_in" => signin_form_response(),
        "/users/local_hosting" => local_hosting_response(),
        "/users/host_dashboard" => host_dashboard_response(),
        _ if path.starts_with("/assets/") => serve_asset_response(request_line),
        _ => cipher_home_response(), // Default to home for now
    }
}

fn signup_form_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>🔐 Cipher - Sign Up</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
        <style>\
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 20px; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; color: white; }\
            .container { max-width: 400px; margin: 0 auto; background: rgba(255, 255, 255, 0.1); padding: 40px; border-radius: 20px; backdrop-filter: blur(10px); box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3); }\
            h1 { font-size: 2.5em; margin-bottom: 30px; text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.3); text-align: center; }\
            .form-group { margin-bottom: 20px; }\
            label { display: block; margin-bottom: 8px; font-weight: 500; }\
            input { width: 100%; padding: 12px; border: none; border-radius: 8px; background: rgba(255, 255, 255, 0.9); color: #333; }\
            button { width: 100%; padding: 12px; border: none; border-radius: 8px; background: #4CAF50; color: white; font-weight: bold; margin-top: 10px; }\
            .link { text-align: center; margin-top: 20px; }\
            a { color: #FFD700; text-decoration: none; }\
        </style>\
    </head>\
    <body>\
        <div class=\"container\">\
            <h1>🔐 Sign Up</h1>\
            <form>\
                <div class=\"form-group\">\
                    <label>Username:</label>\
                    <input type=\"text\" name=\"username\" required>\
                </div>\
                <div class=\"form-group\">\
                    <label>Email:</label>\
                    <input type=\"email\" name=\"email\" required>\
                </div>\
                <div class=\"form-group\">\
                    <label>Password:</label>\
                    <input type=\"password\" name=\"password\" required>\
                </div>\
                <button type=\"submit\">Create Account</button>\
            </form>\
            <div class=\"link\">\
                <a href=\"/users/sign_in\">Already have an account? Sign in</a>\
            </div>\
            <div class=\"link\">\
                <a href=\"/\">← Back to Home</a>\
            </div>\
        </div>\
    </body>\
    </html>".to_string()
}

fn signin_form_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>🔐 Cipher - Sign In</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
        <style>\
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 20px; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; color: white; }\
            .container { max-width: 400px; margin: 0 auto; background: rgba(255, 255, 255, 0.1); padding: 40px; border-radius: 20px; backdrop-filter: blur(10px); box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3); }\
            h1 { font-size: 2.5em; margin-bottom: 30px; text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.3); text-align: center; }\
            .form-group { margin-bottom: 20px; }\
            label { display: block; margin-bottom: 8px; font-weight: 500; }\
            input { width: 100%; padding: 12px; border: none; border-radius: 8px; background: rgba(255, 255, 255, 0.9); color: #333; }\
            button { width: 100%; padding: 12px; border: none; border-radius: 8px; background: #2196F3; color: white; font-weight: bold; margin-top: 10px; }\
            .link { text-align: center; margin-top: 20px; }\
            a { color: #FFD700; text-decoration: none; }\
        </style>\
    </head>\
    <body>\
        <div class=\"container\">\
            <h1>🔐 Sign In</h1>\
            <form>\
                <div class=\"form-group\">\
                    <label>Username:</label>\
                    <input type=\"text\" name=\"username\" required>\
                </div>\
                <div class=\"form-group\">\
                    <label>Password:</label>\
                    <input type=\"password\" name=\"password\" required>\
                </div>\
                <button type=\"submit\">Sign In</button>\
            </form>\
            <div class=\"link\">\
                <a href=\"/users/sign_up\">Need an account? Sign up</a>\
            </div>\
            <div class=\"link\">\
                <a href=\"/\">← Back to Home</a>\
            </div>\
        </div>\
    </body>\
    </html>".to_string()
}

fn serve_asset_response(request_line: &str) -> String {
    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAsset not found"
        .to_string()
}

fn local_hosting_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>🔐 Cipher - Local Hosting</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
    </head>\
    <body>\
        <h1>🏠 Local Hosting</h1>\
        <p>Configure your device to contribute storage to the network and earn CPH tokens.</p>\
        <a href=\"/\">← Back to Home</a>\
    </body>\
    </html>".to_string()
}

fn host_dashboard_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>🔐 Cipher - Host Dashboard</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
    </head>\
    <body>\
        <h1>📊 Host Dashboard</h1>\
        <p>Monitor your hosting contributions and token earnings.</p>\
        <a href=\"/\">← Back to Home</a>\
    </body>\
    </html>".to_string()
}


fn cipher_home_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>🔐 Cipher - Secure Decentralized Communication</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
        <link rel=\"stylesheet\" href=\"/assets/application.css\">\
        <style>\
            /* Rails CSS will be embedded here - using simplified responsive design */\
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 0; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; color: white; }\
            .container { max-width: 1200px; margin: 0 auto; padding: 20px; }\
            .welcome-hero { text-align: center; padding: 40px 0; }\
            .welcome-hero h2 { font-size: 3em; margin-bottom: 20px; text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.3); }\
            .getting-started { margin-top: 30px; }\
            .btn { display: inline-block; padding: 15px 30px; margin: 10px; text-decoration: none; border-radius: 8px; font-weight: bold; transition: all 0.3s ease; }\
            .btn-primary { background: #4CAF50; color: white; }\
            .btn-secondary { background: #2196F3; color: white; }\
            .btn-large { font-size: 1.2em; padding: 20px 40px; }\
            .features { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 30px; margin: 60px 0; }\
            .feature { background: rgba(255, 255, 255, 0.1); padding: 30px; border-radius: 15px; backdrop-filter: blur(10px); }\
            .feature h3 { font-size: 1.5em; margin-bottom: 15px; }\
            .token-economy, .mission-statement { background: rgba(255, 255, 255, 0.1); padding: 40px; margin: 40px 0; border-radius: 20px; backdrop-filter: blur(10px); }\
            .glass-card { background: rgba(255, 255, 255, 0.1); border-radius: 15px; padding: 20px; margin: 20px 0; backdrop-filter: blur(10px); }\
            .glass-card--strong { background: rgba(255, 255, 255, 0.15); }\
            .glass-card--shadow-lg { box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3); }\
            .glass-card--shadow-xl { box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4); }\
            .token-mechanics { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; }\
            .token-section { display: flex; align-items: center; gap: 20px; }\
            .token-icon { font-size: 3em; }\
            .token-benefits { margin: 15px 0; }\
            .token-actions { display: flex; gap: 15px; flex-wrap: wrap; margin-top: 20px; }\
            .btn-primary-token { background: #4CAF50; color: white; padding: 12px 24px; border-radius: 8px; text-decoration: none; font-weight: bold; }\
            .btn-secondary-token { background: #2196F3; color: white; padding: 12px 24px; border-radius: 8px; text-decoration: none; font-weight: bold; }\
            .btn-outline-token { background: transparent; color: white; border: 2px solid white; padding: 12px 24px; border-radius: 8px; text-decoration: none; font-weight: bold; }\
            .mission-principles { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 20px; margin: 30px 0; }\
            .principle { display: flex; align-items: center; gap: 15px; padding: 20px; background: rgba(255, 255, 255, 0.1); border-radius: 10px; }\
            .principle-icon { font-size: 2em; }\
            .mission-call-to-action { text-align: center; margin-top: 40px; }\
            .mission-links { display: flex; gap: 15px; justify-content: center; flex-wrap: wrap; margin-top: 20px; }\
            .btn-outline { background: transparent; color: white; border: 2px solid white; }\
            @media (max-width: 768px) {\
                .container { padding: 15px; }\
                .welcome-hero h2 { font-size: 2em; }\
                .features { grid-template-columns: 1fr; }\
                .token-mechanics { grid-template-columns: 1fr; }\
                .mission-principles { grid-template-columns: 1fr; }\
                .token-section { flex-direction: column; text-align: center; }\
                .principle { flex-direction: column; text-align: center; }\
            }\
        </style>\
    </head>\
    <body>\
        <div class=\"container\">\
            <div class=\"welcome-hero\">\
                <h2>🔐 Cipher</h2>\
                <p>Secure, decentralized communication for the modern web</p>\
                <div class=\"getting-started\">\
                    <a href=\"/users/new\" class=\"btn btn-primary btn-large\">Create Your Identity</a>\
                </div>\
            </div>\
            \
            <div class=\"features\">\
                <div class=\"feature\">\
                    <h3>🔐 End-to-End Encryption</h3>\
                    <p>Messages are encrypted before sending. Only you and your recipients can read them.</p>\
                </div>\
                <div class=\"feature\">\
                    <h3>🌐 Peer-to-Peer</h3>\
                    <p>Direct connections between users. No servers required.</p>\
                </div>\
                <div class=\"feature\">\
                    <h3>🔑 Self-Sovereign Identity</h3>\
                    <p>You control your identity. No external authorities involved.</p>\
                </div>\
                <div class=\"feature\">\
                    <h3>📱 Decentralized</h3>\
                    <p>No central authority. Your communications stay independent.</p>\
                </div>\
            </div>\
            \
            <section class=\"token-economy\">\
                <div class=\"cph-hero-well glass-card glass-card--strong glass-card--shadow-xl\">\
                    <h2>🛡️ Cipher Token (CPH)</h2>\
                    <div class=\"token-intro\">\
                        <p class=\"token-lead\">A lightweight system to prevent network abuse. Contribute storage to receive tokens, use tokens for content access.</p>\
                    </div>\
                </div>\
                \
                <div class=\"token-mechanics\">\
                    <div class=\"token-section glass-card glass-card--strong glass-card--shadow-lg\">\
                        <div class=\"token-icon\">🏠</div>\
                        <div class=\"token-info\">\
                            <h3>Contribute by Hosting</h3>\
                            <p>Provide secure storage for encrypted files and receive CPH tokens for abuse prevention.</p>\
                            <ul class=\"token-benefits\">\
                                <li>Receive tokens for network participation</li>\
                                <li>Help prevent network abuse</li>\
                                <li>Support system sustainability</li>\
                            </ul>\
                        </div>\
                    </div>\
                    \
                    <div class=\"token-section glass-card glass-card--strong glass-card--shadow-lg\">\
                        <div class=\"token-icon\">👀</div>\
                        <div class=\"token-info\">\
                            <h3>Access Content</h3>\
                            <p>Access files and media using CPH tokens to prevent abuse at 1 CPH per KB.</p>\
                            <ul class=\"token-benefits\">\
                                <li>1 CPH = 1 KB of data</li>\
                                <li>Usage-based access control</li>\
                                <li>Prevents spam and abuse</li>\
                            </ul>\
                        </div>\
                    </div>\
                </div>\
                \
                <div class=\"token-getting-started glass-card glass-card--strong\">\
                    <h3>Ready to Join the Network?</h3>\
                    <p>Start contributing to the decentralized storage network to receive CPH tokens.</p>\
                    <div class=\"token-actions\">\
                        <a href=\"/users/local_hosting\" class=\"btn-primary-token\">Start Local Hosting</a>\
                        <a href=\"/users/host_dashboard\" class=\"btn-secondary-token\">Become a Network Host</a>\
                        <a href=\"#\" class=\"btn-outline-token\">Connect Wallet</a>\
                    </div>\
                </div>\
            </section>\
            \
            <section class=\"mission-statement glass-card glass-card--strong glass-card--shadow-xl\">\
                <h2>🛡️ Our Mission</h2>\
                <p class=\"mission-lead\">Private messaging without surveillance or data collection.</p>\
                \
                <div class=\"mission-principles\">\
                    <div class=\"principle\">\
                        <div class=\"principle-icon\">🚫</div>\
                        <div class=\"principle-content\">\
                            <h3>No Servers, No Surveillance</h3>\
                            <p>Messages go directly between users. No servers store your conversations.</p>\
                        </div>\
                    </div>\
                    \
                    <div class=\"principle\">\
                        <div class=\"principle-icon\">🔒</div>\
                        <div class=\"principle-content\">\
                            <h3>No Data Harvesting</h3>\
                            <p>We can't read your messages or track you. Your data stays yours.</p>\
                        </div>\
                    </div>\
                    \
                    <div class=\"principle\">\
                        <div class=\"principle-icon\">📖</div>\
                        <div class=\"principle-content\">\
                            <h3>Open Source Transparency</h3>\
                            <p>All code is public. Audit it, fork it, improve it.</p>\
                        </div>\
                    </div>\
                    \
                    <div class=\"principle\">\
                        <div class=\"principle-icon\">❤️</div>\
                        <div class=\"principle-content\">\
                            <h3>Connect with Loved Ones</h3>\
                            <p>Share files and messages privately with friends and family.</p>\
                        </div>\
                    </div>\
                </div>\
                \
                <div class=\"mission-call-to-action glass-card glass-card--strong glass-card--shadow-lg\">\
                    <p class=\"mission-cta-text\"><strong>Private messaging by design.</strong> No tracking, no data collection, no surveillance.</p>\
                    <div class=\"mission-links\">\
                        <a href=\"https://github.com/anthropics/cipher\" target=\"_blank\" class=\"btn btn-outline\">📖 View Source Code</a>\
                        <a href=\"/users/new\" class=\"btn btn-secondary\">🚀 Get Started</a>\
                    </div>\
                </div>\
            </section>\
        </div>\
    </body>\
    </html>".to_string()
}

fn fallback_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
    <!DOCTYPE html>\
    <html>\
    <head>\
        <title>Cipher P2P Social Network</title>\
        <meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
        <style>\
            body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }\
            .container { max-width: 600px; margin: 0 auto; background: white; padding: 40px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }\
            h1 { color: #333; text-align: center; margin-bottom: 20px; }\
            p { color: #666; line-height: 1.6; text-align: center; }\
            .status { background: #ffe8e8; padding: 15px; border-radius: 6px; color: #d32f2f; margin: 20px 0; }\
        </style>\
    </head>\
    <body>\
        <div class=\"container\">\
            <h1>🔐 Cipher</h1>\
            <div class=\"status\">⚠️ Rails server starting...</div>\
            <p>Your personal Cipher server is starting up.</p>\
            <p>Please wait a moment and refresh the page.</p>\
        </div>\
    </body>\
    </html>".to_string()
}

fn error_response(error: &str) -> String {
    format!(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
        <!DOCTYPE html><html><head><title>Server Error</title></head>\
        <body><h1>Server Error</h1><p>{}</p></body></html>",
        error
    )
}

#[tauri::command]
pub fn get_platform() -> String {
    if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "ios") {
        "ios".to_string()
    } else {
        "mobile".to_string()
    }
}

#[cfg(mobile)]
#[tauri::mobile_entry_point]
pub fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app: &mut tauri::App<tauri::Wry>| {
            println!("Starting Cipher mobile app setup");

            match start_local_embedded_server(app) {
                Ok(()) => {
                    println!("Successfully started local embedded server");

                    // Wait a moment for server to be ready, then redirect webview
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    // Get the main window and navigate to our local server
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval("window.location.href = 'http://127.0.0.1:3000';");
                        println!("Redirected webview to local embedded server");
                    }
                }
                Err(err) => {
                    println!("Failed to start local embedded server: {}", err);
                    // Don't panic - just continue
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_platform])
        .run(tauri::generate_context!())
        .expect("error while running tauri mobile application");
}
