use dlogon::drain::Drain;

fn main() {
    let logs = vec![
        "connected to 10.0.0.1",
        "connected to 10.0.0.2",
        "connected to 10.0.0.3",
        "Hex number 0xDEADBEAF",
        "Hex number 0x10000",
        "user davidoh logged in",
        "user eranr logged in",
    ];
    let mut drain = Drain::default();
    for log in logs {
        drain.train(log);
    }
    println!("{:?}", drain.clusters());
}
