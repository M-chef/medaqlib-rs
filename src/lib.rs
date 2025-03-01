use std::{
    error::Error, ffi::CString, fmt::{Debug, Display}, iter, net::Ipv4Addr, sync::LazyLock, vec
};

#[allow(dead_code, non_camel_case_types, non_snake_case)]
// mod bindings {
//     include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
// }
mod bindings;

pub use bindings::ME_SENSOR;
use bindings::*;

const MEDAQLIB_DLL: &str = "MEDAQLib.dll";
static MEDAQLIB: LazyLock<MEDAQLib> = LazyLock::new(|| unsafe {
    MEDAQLib::new(MEDAQLIB_DLL).expect("could not find dll")
});

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
        let sensor_handle = unsafe { MEDAQLIB.CreateSensorInstance(sensor_type) };
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
        unsafe { MEDAQLIB.OpenSensor(self.sensor_handle).into() }
    }

    fn set_parameter_string(
        &self,
        param_name: CString,
        param_value: CString,
    ) -> Result<(), Box<dyn Error>> {
        let param_name = param_name.as_ptr();
        let param_value = param_value.as_ptr();

        unsafe { MEDAQLIB.SetParameterString(self.sensor_handle, param_name, param_value).into() }
    }

    fn set_parameter_int(
        &self,
        param_name: CString,
        param_value: bool,
    ) -> Result<(), Box<dyn Error>> {
        let param_name = param_name.as_ptr();
        let param_value = param_value as i32;

        unsafe { MEDAQLIB.SetParameterInt(self.sensor_handle, param_name, param_value).into() }
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
            MEDAQLIB.ExecSCmd(self.sensor_handle, sensor_command).to_result()?;
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
                if MEDAQLIB.GetParameterString(self.sensor_handle, param_name, return_value_ptr, max_len)
                    .to_result().is_ok() {
                        CString::from_raw(return_value_ptr)
                    } else {
                        CString::new("").expect("could not create cstring")
                    }
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
            MEDAQLIB.DataAvail(self.sensor_handle, avail).to_result()?;
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
    ///         println!("First: {:?}", data.get_first_scaled());
    ///         println!("Mean: {:?}", data.get_mean_scaled())
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

            MEDAQLIB.TransferData(
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
            MEDAQLIB.CloseSensor(self.sensor_handle);
            MEDAQLIB.ReleaseSensorInstance(self.sensor_handle);
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

#[derive(Debug, Clone)]
pub struct Data {
    pub channels: Vec<String>,
    pub raw_data: Vec<i32>,
    pub scaled_data: Vec<f64>,
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
    pub value: Value<T>,
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

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value<T> {
    Valid(T),
    OutOfRange
}

impl<T: Display> Display for Value<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            Value::Valid(v) => v.to_string(),
            Value::OutOfRange => "OutOfRange".into(),
        };
        write!(f, "{}", val)
    }
}

impl<T: Default> Default for Value<T> {
    fn default() -> Self {
        Value::Valid(T::default())
    }
}

impl<T> Value<T> {
    pub fn into_raw(self) -> Option<T> {
        match self {
            Value::Valid(value) => Some(value),
            Value::OutOfRange => None,
        }
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
        let number_of_channels = channels.len();

        let mut values_means = vec![0.; number_of_channels];
        let mut counts = vec![0; number_of_channels];
        for (i, &value) in self.iter().enumerate() {
            let idx = i % number_of_channels;
            let current_mean = values_means.get_mut(idx).unwrap();
            let current_count = counts.get_mut(idx).unwrap();
            let value: f64 = value.into();
            if value > 0. {
                *current_count += 1;
                *current_mean = *current_mean + (value - *current_mean) / *current_count as f64;
            }
        }

        values_means.iter().zip(channels).zip(counts).map(|((&value, channel), counts)| {
            let value = if counts == 0 {
                Value::OutOfRange
            } else {
                Value::Valid(value)
            };
            ChannelValue { channel, value}
        }).collect()
    }

    fn get_first(&'a self, channels: &'a [String]) -> Vec<ChannelValue<'a, T>> {
        let values = &self[0..channels.len()];
        values.iter().zip(channels).map(|(&value, channel)| {
            let value = match value {
                v if v.into() < 0. => Value::OutOfRange,
                v => Value::Valid(v)
            };
            ChannelValue { channel, value}
        }).collect()
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
            ChannelValue {channel: "1", value: crate::Value::Valid(1)},
            ChannelValue {channel: "2", value: crate::Value::Valid(2)},
            ChannelValue {channel: "3", value: crate::Value::Valid(3)}
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
            ChannelValue {channel: "1", value: crate::Value::Valid(7. / 3.)},
            ChannelValue {channel: "2", value: crate::Value::Valid(10. / 3.)},
            ChannelValue {channel: "3", value: crate::Value::Valid(13. / 3.)}
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
            ChannelValue {channel: "1", value: crate::Value::Valid(7. / 3.)},
            ChannelValue {channel: "2", value: crate::Value::Valid(10. / 3.)},
            ChannelValue {channel: "3", value: crate::Value::Valid(13. / 3.)}
        ])
    }

    #[test]
    fn test_get_mean_scaled_all_out_of_range_test() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![],
            scaled_data: vec![-1.7976931348623157e308, 2., 3., -1.7976931348623157e308, 5., 6., -1.7976931348623157e308, 3., 4.],
        };
        let means = data.get_mean_scaled();
        assert_eq!(means, vec![
            ChannelValue {channel: "1", value: crate::Value::OutOfRange},
            ChannelValue {channel: "2", value: crate::Value::Valid(10. / 3.)},
            ChannelValue {channel: "3", value: crate::Value::Valid(13. / 3.)}
        ])
    }

    #[test]
    fn test_get_mean_scaled_some_out_of_range_test() {
        let data = Data {
            channels: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            raw_data: vec![],
            scaled_data: vec![-1.7976931348623157e308, 2., 3., 1., 5., 6., 1., 3., 4.],
        };
        let means = data.get_mean_scaled();
        assert_eq!(means, vec![
            ChannelValue {channel: "1", value: crate::Value::Valid(1.)},
            ChannelValue {channel: "2", value: crate::Value::Valid(10. / 3.)},
            ChannelValue {channel: "3", value: crate::Value::Valid(13. / 3.)}
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
