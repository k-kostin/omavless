// SPDX-License-Identifier: MIT

use super::*;
use crate::developer_subscription_schedule::DeveloperSubscriptionSchedule;
use crate::subscription_schedule_attempt::{AttemptState, read_attempt};
use crate::subscription_schedule_plan::MIN_INTERVAL_SECS;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicU64;

fn schedule_call(paths: &RuntimePaths, instance: &str, interval: Option<(u64,u64)>) -> Value {
    let (method,params)=if let Some((revision,interval))=interval {
        ("developer.subscription_schedule.set",json!({"instanceId":instance,"expectedPreferenceRevision":revision,"intervalSecs":interval}))
    } else {("developer.subscription_schedule.get",json!({"instanceId":instance}))};
    call(paths,method,params).unwrap()
}

fn wait_schedule(paths: &RuntimePaths, instance: &str, state: &str) -> Value {
    let deadline=std::time::Instant::now()+Duration::from_secs(4);
    loop {
        let result=schedule_call(paths,instance,None);
        assert_eq!(result["ok"],true);
        if result["result"]["attempt"]["state"]==state {return result;}
        assert!(std::time::Instant::now()<deadline,"automatic terminal did not arrive");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn developer_schedule_socket_happy_path_uses_same_owner_supervisor_and_real_http() {
    let base=temporary_base("automatic-socket");
    let (owner,cutover,host_calls)=native_owner_fixture(&base);
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    let store=base.join("config/profiles.json");
    let mut document:Value=serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    document["subscriptions"][0]["url"]=json!(format!("http://{}/synthetic-feed",listener.local_addr().unwrap()));
    fs::write(&store,serde_json::to_vec(&document).unwrap()).unwrap();
    let initial_host_calls=host_calls.load(Ordering::Relaxed);
    let (arrived_tx,arrived_rx)=std::sync::mpsc::sync_channel(1);
    let (release_tx,release_rx)=std::sync::mpsc::sync_channel(1);
    let uid=Uid::current().as_raw();
    let mut server=RuntimeServer::bind_with_owner_factory(RuntimePaths::below(&base.join("runtime")),move |_|Ok(owner)).unwrap();
    let instance=server.instance_id.clone();
    let http_instance=instance.clone();
    let http_cutover=cutover.clone();
    let http=thread::spawn(move || {
        let (mut stream,_)=listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut request=[0u8;1024]; assert!(stream.read(&mut request).unwrap()>0);
        assert_eq!(read_attempt(&http_cutover,uid,1,&http_instance).unwrap().unwrap().state,AttemptState::StartedInCurrentInstance);
        arrived_tx.send(()).unwrap();release_rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let body="vless://22222222-2222-4222-8222-222222222222@192.0.2.2:443?security=none&type=tcp#Synthetic";
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    });
    let now=Arc::new(AtomicU64::new(100));let wake=Arc::new(AtomicBool::new(false));
    let clock=Arc::clone(&now);let wakeup=Arc::clone(&wake);
    server.register_developer_subscription_schedule(DeveloperSubscriptionSchedule::new(
        move ||clock.load(Ordering::Acquire),move ||wakeup.swap(false,Ordering::AcqRel))).unwrap();
    let paths=server.paths.clone();let stop=Arc::new(AtomicBool::new(false));let stop_worker=Arc::clone(&stop);
    let runtime=thread::spawn(move ||server.serve_until(&stop_worker).unwrap());
    assert_eq!(schedule_call(&paths,&instance,None)["result"]["intervalSecs"],0);
    assert_eq!(schedule_call(&paths,&instance,Some((0,MIN_INTERVAL_SECS)))["ok"],true);
    wake.store(true,Ordering::Release);
    arrived_rx.recv_timeout(Duration::from_secs(4)).unwrap();
    let status=call(&paths,"status.get",json!({})).unwrap();assert_eq!(status["ok"],true);assert_eq!(status["revision"],0);
    assert_eq!(schedule_call(&paths,&instance,None)["result"]["workerRegistered"],true);
    release_tx.send(()).unwrap();
    let completed=wait_schedule(&paths,&instance,"succeeded");
    assert_eq!(completed["revision"],1);assert_eq!(completed["result"]["workerRegistered"],false);
    assert_eq!(host_calls.load(Ordering::Relaxed),initial_host_calls);
    let after=fs::read(&store).unwrap();
    now.store(100+MIN_INTERVAL_SECS-1,Ordering::Release);wake.store(true,Ordering::Release);
    thread::sleep(Duration::from_millis(50));
    assert_eq!(schedule_call(&paths,&instance,None)["result"]["attempt"]["sequence"],1);
    assert_eq!(fs::read(&store).unwrap(),after);
    stop.store(true,Ordering::Release);runtime.join().unwrap();http.join().unwrap();
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn developer_schedule_normal_registration_has_no_method_or_worker() {
    let base=temporary_base("automatic-default");let (owner,cutover,_)=native_owner_fixture(&base);
    let server=RuntimeServer::bind_with_owner_factory(RuntimePaths::below(&base.join("runtime")),move |_|Ok(owner)).unwrap();
    let capabilities=server.dispatch(&make_request("caps","capabilities.get",json!({})).unwrap()).unwrap();
    assert!(!capabilities["result"]["methods"].as_array().unwrap().iter().any(|method|method.as_str().unwrap().starts_with("developer.")));
    let response=server.dispatch(&make_request("get","developer.subscription_schedule.get",json!({"instanceId":server.instance_id})).unwrap()).unwrap();
    assert_eq!(response["error"]["code"],"unknown_method");
    assert!(!cutover.state_directory.join("subscription-refresh-preference.json").exists());
    assert!(!cutover.state_directory.join("subscription-refresh-attempt.json").exists());
    drop(server);fs::remove_dir_all(base).unwrap();
}
