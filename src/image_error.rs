use thiserror::Error;


/// WordCountError enumerates all possible errors returned by this library.
#[derive(Error, Debug)]
pub enum MyImageError {   
	
	//Probably retriving the image file
    #[error("{0}")]
    FileError(String),
    
    //A valid image but we only accept images of a minimum size.
    #[error("{0}")]
    ImageTooSmall(String),
    
    //The image library couldn't decode the file as an image
    #[error("{0}")]
    DecodeFail(String),
}
