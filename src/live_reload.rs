use futures_util::{SinkExt, StreamExt, TryFutureExt};
use std::{
    collections::HashMap,
    io::Error,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{
    filters::{
        path::FullPath,
        ws::{Message, WebSocket},
    },
    Filter,
};

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);
pub type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

pub fn live_reload_thread(adr: SocketAddr) -> Result<(Users, std::thread::JoinHandle<()>), Error> {
    let users = Users::default();

    let u = users.clone();
    let handle = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { live_reload_init(users, adr).await });
    });

    Ok((u, handle))
}

async fn live_reload_init<S: Into<SocketAddr>>(users: Users, addr: S) {
    pretty_env_logger::init();

    let users = warp::any().map(move || users.clone());

    let live_reload = warp::path("live-reload")
        .and(warp::ws())
        .and(users)
        .map(|ws: warp::ws::Ws, users| ws.on_upgrade(move |socket| user_connected(socket, users)));

    let full = warp::path::full().map(|full: FullPath| {
        println!("{}", full.as_str());
        if let Ok(s) = std::fs::read_to_string("out/404.html") {
            return warp::reply::html(s);
        } else {
            return warp::reply::html("Page not found".to_string());
        }
    });

    let dir = warp::fs::dir("out");
    let routes = live_reload.or(dir.or(full));

    warp::serve(routes).run(addr).await;
}

async fn user_connected(ws: WebSocket, users: Users) {
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                })
                .await;
        }
    });

    users.write().await.insert(my_id, tx);

    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error(uid={}): {}", my_id, e);
                break;
            }
        };
        user_message(my_id, msg, &users).await;
    }

    user_disconnected(my_id, &users).await;
}

async fn user_message(my_id: usize, msg: Message, users: &Users) {
    let msg = if let Ok(s) = msg.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("<User#{}>: {}", my_id, msg);

    for (&uid, tx) in users.read().await.iter() {
        if my_id != uid {
            if let Err(_disconnected) = tx.send(Message::text(new_msg.clone())) {}
        }
    }
}

async fn user_disconnected(my_id: usize, users: &Users) {
    users.write().await.remove(&my_id);
}

pub async fn update(users: &Users) {
    for u in users.read().await.values() {
        u.send(Message::text("reload")).unwrap();
    }
}
