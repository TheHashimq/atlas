use axum::{
    http::{header, Method, StatusCode},
    response::IntoResponse,
    Router,
    ServiceExt,
};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Host to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Directory to serve
    #[arg(short, long, default_value = ".")]
    dir: PathBuf,
}

#[tokio::main]
async fn main() {
    // 1. Parse CLI arguments
    let args = Args::parse();

    // 2. Initialize logging with a nice format
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "atlas_server=info,tower_http=debug");
    }
    env_logger::builder()
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .init();

    let root = std::env::current_dir().unwrap().join(&args.dir);
    
    print_banner(&args, &root);

    // 3. Build our application with routes
    let app = Router::new()
        // Serve static files
        .fallback_service(ServeDir::new(&root).append_index_html_on_directories(true))
        // Add middleware
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([header::CONTENT_TYPE]),
        )
        .layer(TraceLayer::new_for_http());

    // 4. Run it
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .expect("Invalid address");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_error(err: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", err),
    )
}

fn print_banner(args: &Args, root: &std::path::Path) {
    println!("\x1b[1;36m");
    println!("     ▄▀█ ▀█▀ █   ▄▀█ █▀");
    println!("     █▀█  █  █▄▄ █▀█ ▄█");
    println!("                       ");
    println!("  \x1b[0m\x1b[1mATLAS Engine Development Server\x1b[0m");
    println!("  ----------------------------------------");
    println!("  \x1b[1;32mHost:\x1b[0m    http://{}", format!("{}:{}", args.host, args.port));
    println!("  \x1b[1;32mRoot:\x1b[0m    {}", root.display());
    println!("  \x1b[1;32mLogs:\x1b[0m    Enabled (Tracing)");
    println!("  ----------------------------------------");
    println!();
}
