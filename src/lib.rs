use std::{
    error::Error, ffi::CString, fmt::{Debug, Display}, iter, net::Ipv4Addr, sync::mpsc::channel, vec
};

#[allow(dead_code, non_camel_case_types, non_snake_case)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::ME_SENSOR;
use bindings::*;

/// Builder for creating new Sensor instance and connect to it
///
/// # Example
/// ```
/// use medaq_lib::{Interface, SensorBuilder, ME_SENSOR};
///
/// let sensor = SensorBuilder::new(ME_SENSOR::SENSOR_IFD2421)
///     .with_interface(Interface::TcpIp)
///     .with_ip_address("10.10.10.10")
///     .enable_logging()
///     .connect()
///     .unwrap();
///

#[derive(Default)]
pub struct SensorBuilder {
    sensor_handle: u32,
    interface: Option<Interface>,
    ip_address: Option<String>,
    logging: bool,
}

impl SensorBuilder {
    pub fn new(sensor_type: ME_SENSOR) -> Self {
        let sensor_handle = unsafe { CreateSensorInstance(sensor_type) };
        Self {
            sensor_handle,
            ..Default::default()
        }
    }

    /// Select the interface to be used
    pub fn with_interface(self, interface: Interface) -> Self {
        let interface = Some(interface);
        Self { interface, ..self }
    }

    pub fn with_ip_address(self, ip_address: impl Into<String>) -> Self {
        let ip_address = Some(ip_address.into());
        Self { ip_address, ..self }
    }

    /// enable Logfile writing
    pub fn enable_logging(self) -> Self {
        Self {
            logging: true,
            ..self
        }
    }

    pub fn connect(self) -> Result<Sensor, Box<dyn Error>> {
        let interface = self.interface.ok_or("no interface provided")?;
        self.set_interface(&interface)?;

        let ip_address = self
            .ip_address
            .as_ref()
            .ok_or("no ip address provided")?
            .parse()?;
        self.set_ip_address(&ip_address)?;

        if self.logging {
            self.set_enable_logging()?;
        }

        self.open_sensor()?;

        let mut sensor = Sensor {
            sensor_handle: self.sensor_handle,
            parameters: vec![],
        };

        sensor.get_parameters()?;

        Ok(sensor)
    }

    fn set_interface(&self, interface: &Interface) -> Result<(), Box<dyn Error>> {
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
        unsafe { OpenSensor(self.sensor_handle).into() }
    }

    fn set_parameter_string(
        &self,
        param_name: CString,
        param_value: CString,
    ) -> Result<(), Box<dyn Error>> {
        let param_name = param_name.as_ptr();
        let param_value = param_value.as_ptr();

        unsafe { SetParameterString(self.sensor_handle, param_name, param_value).into() }
    }

    fn set_parameter_int(
        &self,
        param_name: CString,
        param_value: bool,
    ) -> Result<(), Box<dyn Error>> {
        let param_name = param_name.as_ptr();
        let param_value = param_value as i32;

        unsafe { SetParameterInt(self.sensor_handle, param_name, param_value).into() }
    }
}

#[derive(Debug)]
pub struct Sensor {
    sensor_handle: u32,
    parameters: Vec<String>,
}

impl Sensor {
    fn get_parameters(&mut self) -> Result<(), Box<dyn Error>> {
        let sensor_command =
            CString::new("Get_TransmittedDataInfo").expect("could not create cstring");
        let mut counter = 0;

        let param_names_repeater = iter::repeat_with(|| {
            counter += 1;
            CString::new(format!("IA_Scaled_Name{counter}")).expect("could not create cstring")
        });

        let sensor_command = sensor_command.as_ptr();
        unsafe {
            ExecSCmd(self.sensor_handle, sensor_command).to_result()?;
        }

        for param_name in param_names_repeater {
            let param_name = param_name.as_ptr();
            let (c_string, mut max_len) = {
                let s = "                                                                    ";
                let return_value = CString::new(s).expect("could not create cstring");
                let max_len = s.len() as u32;
                (return_value, max_len)
            };
            let return_value_ptr = c_string.into_raw();
            let max_len = &mut max_len as *mut u32;
            let return_value = unsafe {
                GetParameterString(self.sensor_handle, param_name, return_value_ptr, max_len)
                    .to_result()?;
                CString::from_raw(return_value_ptr)
            }
            .into_string()?;

            if return_value.is_empty() {
                break;
            }
            self.parameters.push(return_value);
        }

        Ok(())
    }

    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }

    fn data_available(&self) -> Result<i32, Box<dyn Error>> {
        let mut avail = 0;
        unsafe {
            let avail = &mut avail as *mut i32;
            DataAvail(self.sensor_handle, avail).to_result()?;
        };
        Ok(avail)
    }

    /// Read data from sensor.
    ///
    /// If no data available yet it will return `Ok(None)` otherwise `Ok(Data)`
    ///
    /// # Example
    /// ```
    /// use std::time::Duration;
    /// use medaq_lib::{Interface, SensorBuilder, ME_SENSOR};
    ///
    /// let sensor = SensorBuilder::new(ME_SENSOR::SENSOR_IFD2421)
    ///     .with_interface(Interface::TcpIp)
    ///     .with_ip_address("10.10.10.10")
    ///     .enable_logging()
    ///     .connect()
    ///     .unwrap();
    ///
    /// loop {
    ///     if let Ok(Some(data)) = sensor.read_data() {
    ///         println!("First: {:?}", data.get_first_scaled(7));
    ///         println!("Mean: {:?}", data.get_mean_scaled(7))
    ///     }
    ///     std::thread::sleep(Duration::from_millis(500));
    /// }
    /// ```
    pub fn read_data(&self) -> Result<Option<Data>, Box<dyn Error>> {
        let max_values = self.data_available()?;
        if max_values == 0 {
            return Ok(None);
        }

        let mut raw_data: Vec<i32> = Vec::with_capacity(max_values as usize);
        let mut scaled_data: Vec<f64> = Vec::with_capacity(max_values as usize);

        let mut read = 0;

        unsafe {
            // Ensure the vectors have allocated space by setting their length
            raw_data.set_len(max_values as usize);
            scaled_data.set_len(max_values as usize);

            TransferData(
                self.sensor_handle,
                raw_data.as_mut_ptr(),
                scaled_data.as_mut_ptr(),
                max_values,
                &mut read,
            )
            .to_result()?;

            // Adjust the lengths to the actual number of values read
            raw_data.set_len(read as usize);
            scaled_data.set_len(read as usize);
        };
        Ok(Some(Data {
            channels: self.parameters.clone(),
            raw_data,
            scaled_data,
        }))
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
pub enum Interface {
    RS232,
    If2004Usb,
    If2008,
    If2008Eth,
    TcpIp,
    WinUSB,
}

impl Display for Interface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Interface::RS232 => "RS232",
            Interface::If2004Usb => "IF2004_USB",
            Interface::If2008 => "IF2008",
            Interface::If2008Eth => "IF2008_ETH",
            Interface::TcpIp => "TCP/IP",
            Interface::WinUSB => "WinUSB",
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

impl ERR_CODE {
    fn to_result(self) -> Result<(), Box<dyn Error>> {
        self.into()
    }
}

impl Error for ERR_CODE {}

impl Display for ERR_CODE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct Data {
    channels: Vec<String>,
    raw_data: Vec<i32>,
    scaled_data: Vec<f64>,
}

impl Data {
    /// Get raw values of very first measurement
    pub fn get_first_raw(&self) ->  Vec<ChannelValue<'_, i32>> {
        self.raw_data.get_first(&self.channels)
    }

    /// Calculates mean of raw values for all channels
    pub fn get_mean_raw(&self) -> Vec<ChannelValue<'_, f64>> {
        self.raw_data.means(&self.channels)
    }

    /// Get scaled values of very first measurement
    pub fn get_first_scaled(&self) ->  Vec<ChannelValue<'_, f64>> {
        self.scaled_data.get_first(&self.channels)
    }

    /// Calculates mean of scaled values for all channels
    pub fn get_mean_scaled(&self) ->  Vec<ChannelValue<'_, f64>> {
        self.scaled_data.means(&self.channels)
    }
}

#[derive(Debug, PartialEq)]
pub struct ChannelValue<'a, T> {
    pub channel: &'a str,
    pub value: T,
}

impl<T: Display> Display for ChannelValue<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.channel, self.value)
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let means: Vec<_> = self.get_mean_scaled().iter().map(|ch| ch.to_string()).collect();
        write!(f, "{}", means.join(" "))
        
    }
}

trait DataTransformation<'a, T> {
    fn means(&'a self, channels: &'a[String]) -> Vec<ChannelValue<'a, f64>>;
    fn get_first(&'a self, channels: &'a [String]) -> Vec<ChannelValue<'a, T>>;
}

impl<'a, T: 'a> DataTransformation<'a, T> for Vec<T>
where
    T: Clone + Copy + Into<f64>, 
{
    fn means(&'a self, channels: &'a [String]) -> Vec<ChannelValue<'a, f64>> {
        let len_channels = channels.len();
        let values_mean = self.chunks(len_channels)
            .enumerate()
            .fold(vec![0.; len_channels], |mut acc, (iteration, chunk)| {
                for (curr_mean, new_value) in acc.iter_mut().zip(chunk) {
                    let new_value: f64 = (*new_value).into();
                    let count = (iteration + 1) as f64;
                    *curr_mean = *curr_mean + (new_value - *curr_mean) / count;
                }
                acc
            });
        values_mean.iter().zip(channels).map(|(&value, channel)| ChannelValue { channel, value}).collect()
    }

    fn get_first(&'a self, channels: &'a [String]) -> Vec<ChannelValue<'a, T>> {
        let values = &self[0..channels.len()];
        values.iter().zip(channels).map(|(&value, channel)| ChannelValue { channel, value}).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{ChannelValue, Data};

    #[test]
    fn test_get_first_raw_test() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![1, 2, 3, 4, 5, 6, 2, 3, 4],
            scaled_data: vec![],
        };
        let means = data.get_first_raw();
        assert_eq!(means, vec![
            ChannelValue {channel: "1", value: 1},
            ChannelValue {channel: "2", value: 2},
            ChannelValue {channel: "3", value: 3}
        ])
    }

    #[test]
    fn test_get_mean_raw_test() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![1, 2, 3, 4, 5, 6, 2, 3, 4],
            scaled_data: vec![],
        };
        let means = data.get_mean_raw();
        assert_eq!(means, vec![
            ChannelValue {channel: "1", value: 7. / 3.},
            ChannelValue {channel: "2", value: 10. / 3.},
            ChannelValue {channel: "3", value: 13. / 3.}
        ])
    }

    #[test]
    fn test_get_mean_scaled_test() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![],
            scaled_data: vec![1., 2., 3., 4., 5., 6., 2., 3., 4.],
        };
        let means = data.get_mean_scaled();
        assert_eq!(means, vec![
            ChannelValue {channel: "1", value: 7. / 3.},
            ChannelValue {channel: "2", value: 10. / 3.},
            ChannelValue {channel: "3", value: 13. / 3.}
        ])
    }

    #[test]
    #[ignore = "manual test"]
    fn test_display_data() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![],
            scaled_data: vec![1., 2., 3., 4., 5., 6., 2., 3., 4.],
        };
        println!("{data}");
    }
}
