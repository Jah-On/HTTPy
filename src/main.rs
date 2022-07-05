mod HTTPy;

#[tokio::main]
async fn main() {
    let mut server = HTTPy::HttpServer::new().await;
    server.add_get("/", index);
    server.run().await;
}

pub fn index(data: String) -> String {
    return HTTPy::file("./index.html");
}