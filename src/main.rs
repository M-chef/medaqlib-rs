use std::time::Instant;

use medaqlib::*;

fn main() {
    let sensor = SensorBuilder::new(ME_SENSOR::SENSOR_IFD2411)
        .with_interface(Interface::TcpIp)
        .with_ip_address("169.254.168.150")
        .connect()
        .unwrap();
    let mut instant = Instant::now();
    loop {
        if let Some(data) = sensor.read_data().unwrap() {
            let later = Instant::now();
            let elapsed = later.duration_since(instant);
            dbg!(elapsed);
            instant = later;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
