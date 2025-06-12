pub trait Serializable{
    fn bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &Vec<u8>) -> Self;
}

impl Serializable for u8 {
    fn bytes(&self) -> Vec<u8> {self.to_le_bytes().to_vec()}
    fn from_bytes(bytes: &Vec<u8>) -> u8{ u8::from_le_bytes(bytes[0..=0].try_into().unwrap()) }
}
impl Serializable for u16{
    fn bytes(&self) -> Vec<u8> {self.to_le_bytes().to_vec()}
    fn from_bytes(bytes: &Vec<u8>) -> u16{ u16::from_le_bytes(bytes[0..=1].try_into().unwrap()) }
}
impl Serializable for u32{
    fn bytes(&self) -> Vec<u8> {self.to_le_bytes().to_vec()}
    fn from_bytes(bytes: &Vec<u8>) -> u32{ u32::from_le_bytes(bytes[0..=3].try_into().unwrap()) }
}
impl Serializable for u64{
    fn bytes(&self) -> Vec<u8> {self.to_le_bytes().to_vec()}
    fn from_bytes(bytes: &Vec<u8>) -> u64{ u64::from_le_bytes(bytes[0..=7].try_into().unwrap()) }
}

impl Serializable for String{
    fn bytes(&self) -> Vec<u8> { self.as_bytes().to_vec() }
    fn from_bytes(bytes: &Vec<u8>) -> Self { bytes.iter().map(|&b| b as char).collect::<String> () }
}

/*
pub struct Serializer<T: Serializable>{
    size: fn () -> usize,
    input_bytes: fn (usize) -> Result<Vec<u8>, SockError>
}

impl<T: Serializable> Deserializer<T> {
    pub fn serialize(&self, obj: T) -> Result<Vec<u8>, SockError> {
        obj.bytes()
        match self.obj {
            Some(&obj) => obj.bytes(),
            None => Err(SockError::UnknownError)
        }
    }

    pub fn deserialize(&mut self) -> Result<(), SockError> {
        let obj = T::from_bytes(self.input_bytes(self.size())?);
        self.obj = Some(obj);
        Ok(())
    }
}
*/