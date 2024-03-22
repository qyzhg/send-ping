use futures::channel::oneshot;
use futures::future::{self, Either};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local};
use wasm_timer::{Delay, Instant};
use web_sys::{WebSocket, MessageEvent};

#[wasm_bindgen]
pub fn start_ping_task(url: String, interval: u32, timeout: u32) {
    spawn_local(async move {
        loop {
            if let Err(e) = send_ping(&url, timeout).await {
                web_sys::console::log_1(&format!("发送PING时出错: {:?}", e).into());
            }
            Delay::new(Duration::from_secs(interval as u64)).await.expect("启动ping任务失败");
        }
    });
}

async fn send_ping(url: &str, timeout: u32) -> Result<(), JsValue> {
    let ws = WebSocket::new(url)?;
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let (tx, rx) = oneshot::channel::<Result<(), JsValue>>();
    let tx = Rc::new(RefCell::new(Some(tx)));

    let pong_received = Rc::new(RefCell::new(false));

    let onmessage = {
        let pong_received = pong_received.clone();
        Closure::wrap(Box::new(move |e: MessageEvent| {
            if e.data().as_string().unwrap_or_default() == "PONG" {
                *pong_received.borrow_mut() = true;
            }
        }) as Box<dyn FnMut(MessageEvent)>)
    };
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onerror = {
        let tx = tx.clone();
        Closure::wrap(Box::new(move |e: JsValue| {
            let _ = tx.borrow_mut().take().unwrap().send(Err(e));
        }) as Box<dyn FnMut(JsValue)>)
    };
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();
    let ws_clone_for_open = ws.clone();

    let onopen = Closure::wrap(Box::new(move |_| {
        if ws_clone_for_open.send_with_str("PING").is_ok() {

        }
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let timeout = Delay::new(Duration::from_secs(timeout as u64));

    match future::select(Box::pin(timeout), rx).await {
        Either::Left((_, _)) => {
            if *pong_received.borrow() {
                Ok(())
            } else {
                Err(JsValue::from_str("未收到PONG响应"))
            }
        },
        Either::Right((Ok(Ok(_)), _)) => Ok(()),
        Either::Right((Ok(Err(e)), _)) => Err(e),
        Either::Right((Err(_), _)) => Err(JsValue::from_str("无法从oneshot通道接收响应")),
    }
}
