//! A camera that rejects talkback and permits only one connection attempt.
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};
use watchtower_core::StreamBackend;
use watchtower_media::Media;

#[tokio::test]
#[ignore = "requires fetched sidecar; set WATCHTOWER_TEST_SIDECAR"]
async fn receive_only_camera_needs_no_talkback_or_second_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let camera = tokio::spawn(async move {
        let mut handlers = tokio::task::JoinSet::new();
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let attempt = observed.fetch_add(1, Ordering::SeqCst);
            handlers.spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut read = BufReader::new(read);
                loop {
                    let mut request = String::new();
                    loop {
                        let mut line = String::new();
                        if read.read_line(&mut line).await.unwrap_or(0) == 0 { return; }
                        request.push_str(&line);
                        if line == "\r\n" { break; }
                    }
                    let sequence = request.lines().find_map(|line| line.strip_prefix("CSeq: ")).unwrap_or("1");
                    let method = request.split_whitespace().next().unwrap();
                    let (status, headers, body) = if attempt > 0 {
                        ("453 Not Enough Bandwidth", String::new(), String::new())
                    } else if request.to_ascii_lowercase().contains("require:") {
                        ("551 Option Not Supported", String::new(), String::new())
                    } else if method == "DESCRIBE" {
                        ("200 OK", format!("Content-Type: application/sdp\r\nContent-Base: rtsp://127.0.0.1:{port}/live/\r\n"), "v=0\r\no=- 1 1 IN IP4 127.0.0.1\r\ns=Receive only\r\nc=IN IP4 127.0.0.1\r\nt=0 0\r\nm=video 0 RTP/AVP 96\r\na=rtpmap:96 H264/90000\r\na=control:trackID=0\r\n".into())
                    } else if method == "SETUP" {
                        ("200 OK", "Session: synthetic\r\nTransport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n".into(), String::new())
                    } else {
                        ("200 OK", "Session: synthetic\r\n".into(), String::new())
                    };
                    let response = format!("RTSP/1.0 {status}\r\nCSeq: {sequence}\r\n{headers}Content-Length: {}\r\n\r\n{body}", body.len());
                    if write.write_all(response.as_bytes()).await.is_err() { return; }
                }
            });
        }
    });
    let binary = PathBuf::from(std::env::var("WATCHTOWER_TEST_SIDECAR").expect("sidecar path"));
    let media = Media::launch(&binary).await.unwrap();
    media
        .add_stream("receive_only", &format!("rtsp://127.0.0.1:{port}/live"))
        .await
        .unwrap();
    let result = media.test_stream("receive_only").await;
    media.shutdown().await;
    camera.abort();
    result.expect("receive-only probe must not request talkback");
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
