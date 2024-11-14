use std::{error::Error, ffi::CString, fmt::{Debug, Display}, net::Ipv4Addr};

#[allow(dead_code, non_camel_case_types, non_snake_case)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

// mod bindings;

use bindings::*;
pub use bindings::*;


#[derive(Default)]
pub struct SensorBuilder {
    sensor_handle: u32,
    interface: Option<IpInterface>,
    ip_address: Option<String>,
    logging: bool,
}

impl SensorBuilder {
    

    pub fn new(sensor_type: ME_SENSOR) -> Self {
        let sensor_handle = unsafe {
            CreateSensorInstance(sensor_type)
        };
        dbg!(&sensor_handle);
        Self { sensor_handle, ..Default::default() }
    }

    /// Select the interface to be used
    pub fn with_interface(self, interface: IpInterface) -> Self {
        let interface = Some(interface);
        Self { interface, ..self }
    }

    pub fn with_ip_address(self, ip_address: impl Into<String>) -> Self {
        let ip_address = Some(ip_address.into());
        Self { ip_address, ..self }
    }

    /// enable Logfile writing
    pub fn enable_logging(self) -> Self {
        Self { logging: true, ..self }
    }

    pub fn connect(self) -> Result<Sensor, Box<dyn Error>> {
        let interface = self.interface.ok_or("no interface provided")?;
        dbg!(self.set_interface(&interface))?;

        let ip_address = self.ip_address.as_ref()
            .ok_or("no ip address provided")?
            .parse()?;
        dbg!(self.set_ip_address(&ip_address))?;

        dbg!(self.set_enable_logging()?);

        dbg!(self.open_sensor())?;

        Ok(Sensor {
            sensor_handle: self.sensor_handle
        })
    }

    fn set_interface(&self, interface: &IpInterface) -> Result<(), Box<dyn Error>> {
        let param_name = CString::new("IP_Interface").expect("error creating cstring");
        let param_value = CString::new(interface.to_string()).expect("error creating cstring");
        self.set_parameter_string(param_name, param_value)
    }

    fn set_ip_address(&self, ip_address: &Ipv4Addr) -> Result<(), Box<dyn Error>> {
        let param_name = CString::new("IP_RemoteAddr").expect("error creating cstring");
        let param_value = CString::new(ip_address.to_string()).expect("error creating cstring");
        self.set_parameter_string(param_name, param_value)
    }

    fn set_enable_logging(&self) -> Result<(), Box<dyn Error>> {
        let param_name = CString::new("IP_EnableLogging").expect("error creating cstring");
        let param_value = true;
        self.set_parameter_int(param_name, param_value)
    }
    
    fn open_sensor(&self) -> Result<(), Box<dyn Error>> {
        let result = unsafe {
            OpenSensor(self.sensor_handle)   
        };
        result.into()
    }

    fn set_parameter_string(&self, param_name: CString, param_value: CString) -> Result<(), Box<dyn Error>> {

        let param_name = param_name.as_ptr();
        let param_value = param_value.as_ptr();

        let result = unsafe {
            SetParameterString(self.sensor_handle, param_name, param_value)
        };
        result.into()
    }

    fn set_parameter_int(&self, param_name: CString, param_value: bool) -> Result<(), Box<dyn Error>> {

        let param_name = param_name.as_ptr();
        let param_value = param_value as i32;

        let result = unsafe {
            SetParameterInt(self.sensor_handle, param_name, param_value)
        };
        result.into()
    }
    
    

}


pub struct Sensor {
    sensor_handle: u32,
}

impl Sensor {
    pub fn data_available(&self) -> Result<i32, Box<dyn Error>> {
        let mut avail = 0;
        let result = unsafe {
            let avail = &mut avail as *mut i32;
            DataAvail(self.sensor_handle, avail)
        };
        let result: Result<(), Box<dyn Error>> = result.into();
        result?;
        Ok(avail)
    }


    pub fn read_data(&self, max_values: i32) -> Result<Data, Box<dyn Error>>{
        let mut raw_data: Vec<i32> = Vec::with_capacity(max_values as usize);
        let mut scaled_data: Vec<f64> = Vec::with_capacity(max_values as usize);

        let mut read = 0;
        
        let result = unsafe {
            // Ensure the vectors have allocated space by setting their length
            raw_data.set_len(max_values as usize);
            scaled_data.set_len(max_values as usize);

            let transfer_data = TransferData(self.sensor_handle, raw_data.as_mut_ptr(), scaled_data.as_mut_ptr(), max_values, &mut read);

            // Adjust the lengths to the actual number of values read
            raw_data.set_len(read as usize);
            scaled_data.set_len(read as usize);

            transfer_data
        };
        let result: Result<(), Box<dyn Error>> = result.into();
        result?;
        Ok(Data { raw_data, scaled_data })
    }

}

impl Drop for Sensor {
    fn drop(&mut self) {
        println!("release sensor...");
        unsafe {
            CloseSensor(self.sensor_handle);
            ReleaseSensorInstance(self.sensor_handle);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IpInterface {
    RS232,
    If2004Usb,
    If2008,
    If2008Eth,
    TcpIp,
    WinUSB,
}
    
impl Display for IpInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        
        let s = match self {
            IpInterface::RS232 => "RS232",
            IpInterface::If2004Usb => "IF2004_USB",
            IpInterface::If2008 => "IF2008",
            IpInterface::If2008Eth => "IF2008_ETH",
            IpInterface::TcpIp => "TCP/IP",
            IpInterface::WinUSB => "WinUSB",
        };
        
        write!(f, "{s}")
    }
}

impl From<ERR_CODE> for Result<(), Box<dyn Error>> {
    fn from(value: ERR_CODE) -> Self {
        match value {
            ERR_CODE::ERR_NOERROR => Ok(()),
            err_code => Err(Box::new(err_code)),
        }
    }
}

impl Error for ERR_CODE {}

impl Display for ERR_CODE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}


#[derive(Debug)]
pub struct Data {
    raw_data: Vec<i32>,
    pub scaled_data: Vec<f64>,
}

impl Data {
    
    pub fn get_mean_raw(&self, channels: usize) -> Vec<u64> {
        let sums = self.raw_data.chunks(channels)
            .fold(vec![0u64; channels], |mut acc, chunk| {
                for (curr, new) in acc.iter_mut().zip(chunk) {
                    *curr += *new as u64
                }
                acc
            });

        sums.into_iter()
            .map(|sum| sum / ( self.raw_data.len() as u64 / channels as u64))
            .collect()
    }

    pub fn get_mean_scaled(&self, channels: usize) -> Vec<f64> {
        let sums = self.scaled_data.chunks(channels)
            .fold(vec![0.; channels], |mut acc, chunk| {
                for (curr, new) in acc.iter_mut().zip(chunk) {
                    *curr += new
                }
                acc
            });

        sums.into_iter()
            .map(|sum| sum / ( self.scaled_data.len() as f64 / channels as f64))
            .collect()
    }

}