mod HTTPy;

#[tokio::main]
async fn main() {
    let mut server = HTTPy::HttpServer::new().await;
    server.set_root_dir("/var/www/html/");
    server.handle_all_statics();
    // server.add_get("/", index);
    server.run().await;
}

pub fn index(data: &str) -> String {
    // return HTTPy::ok();
    return HTTPy::file("/var/www/html/index.html");
}