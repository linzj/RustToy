use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let listen_port = "5556";
    let forward_address = "127.0.0.1:5555";

    // Start listening for incoming connections on listen_port.
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", listen_port)).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Error starting TCP listener on port {}: {}", listen_port, e);
            return;
        }
    };

    println!(
        "Listening on port {}, forwarding to {}",
        listen_port, forward_address
    );

    loop {
        // Accept new connections.
        let (mut client_conn, client_addr) =
            listener.accept().await.expect("Error accepting connection");
        println!("Accepted connection from {}", client_addr);

        let forward_address = forward_address.to_string();

        // Handle the connection in an asynchronous task.
        tokio::spawn(async move {
            let mut server_conn = TcpStream::connect(&forward_address)
                .await
                .expect("Error connecting to forward address");

            // Split the TCP streams into read and write halves.
            let (mut client_read, mut client_write) = client_conn.split();
            let (mut server_read, mut server_write) = server_conn.split();

            // Copy data from the client to the server and vice-versa.
            let client_to_server = io::copy(&mut client_read, &mut server_write);
            let server_to_client = io::copy(&mut server_read, &mut client_write);

            // Use tokio::try_join to wait for both copy operations to complete.
            let _ = tokio::try_join!(client_to_server, server_to_client)
                .expect("Error while copying data between client and server");
        });
    }
}
