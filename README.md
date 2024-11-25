# medaqlib-rs - A Rust wrapper for MEDAQLib by micro-epsilon

This is a Rust wrapper for the MEDAQLib C-Library. 

# Usage

Setup the project
- Download the library from micro-epsilon homepage: https://www.micro-epsilon.de/fileadmin/download/software/MEDAQLib.zip
- Place the containing MEDAQLIB.dll where the executable can find it
- add the dependency to your Cargo.toml

```
[dependencies]
medaqlib = { git = "https://github.com/M-chef/medaqlib-rs" }
```

Connect to sensor and read data

```rust
use std::time::Duration;
use medaqlib::{Interface, SensorBuilder, ME_SENSOR};

fn main()

    // Use SensorBuilder to setup sensor connection.
    // Provide the name of your sensor from the ME_SENSOR enum
    let sensor = SensorBuilder::new(ME_SENSOR::SENSOR_IFD2421)
        // setup communication interface
        .with_interface(Interface::TcpIp)
        // setup IP address
        .with_ip_address("10.10.10.10")
        //enable logging
        .enable_logging()
        // open connection to sensor
        .connect()
        .unwrap();

    // read data
    loop {
        if let Ok(Some(data)) = sensor.read_data() {
            println!("First: {:?}", data.get_first_scaled());
            println!("Mean: {:?}", data.get_mean_scaled())
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
```

# Development
- Download the library from micro-epsilon homepage: https://www.micro-epsilon.de/fileadmin/download/software/MEDAQLib.zip
- Place the containing MEDAQLIB.lib, MEDAQLIB.h in the root of project (where the Cargo.toml is located)
- Run `cargo build`
