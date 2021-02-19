extern crate image;

use std::cmp::Ordering;
use image::{GenericImageView, DynamicImage};
use image::imageops::FilterType;
use image::io::Reader;
use std::fs;

use crate::image_error::MyImageError;


pub struct ImageHashAV {
	pub dupe_group : u64 ,		//A common key to group potential duplicates - same integer means possible (but not yet confirmed) dupe
	pub grey_hash : u64,		//A hash code of a greyscale low resolution version of the image
	pub low_res : [u8;192],		//The pixels of a colour low resolution version of the image
	pub width: u32,				//Width of the original image in pixels
	pub height: u32,			//Height of the original image in pixels
	pub file_size : u64,		//File size in bytes
	pub num_pixels: u64,		//Total number of pixels in the original image
	pub std_dev : f32,			//Standard deviation of colour values from the mean (used to avoid testing images with low variation)
	pub fpath: String,			//The path to the image
}

pub struct ConfigOptions {
	pub colour_difference_threshold : u64,
	pub std_dev_threshold : f32,
	pub alg_flip_threshold : u64,
	pub alg_colour_diff_only : bool,
	pub only_known_file_extensions : bool,
	pub only_list_duplicates : bool,
	pub only_list_uniques : bool,
	pub list_all : bool,
	pub num_threads : u32,
}

/**
 * Order the images with the following keys
 * 	1st) The dupe_group (ascending)
 *  2nd) The total number of pixels (descending)
 *  3rd) The file size (descending)
 * 
 */
impl Ord for ImageHashAV {
	
    fn cmp(&self, other: &Self) -> Ordering {

        if self.dupe_group < other.dupe_group{
			return Ordering::Less;
		}
		if self.dupe_group > other.dupe_group{
			return Ordering::Greater;
		}
		
		//Push files with greater number of pixels further up the list
		if self.num_pixels > other.num_pixels{
			return Ordering::Less;
		}
		
		if self.num_pixels < other.num_pixels{
			return Ordering::Greater;
		}
		
		//Push larger file sizes further up the list
		if self.file_size > other.file_size {
			return Ordering::Less;
		}
		
		if self.file_size < other.file_size {
			return Ordering::Greater;
		}
		
		return Ordering::Equal
    }
    
}

impl Eq for ImageHashAV {}

impl PartialOrd for ImageHashAV {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ImageHashAV {
    fn eq(&self, other: &Self) -> bool {
        if (self.dupe_group == other.dupe_group) && 
           ( self.num_pixels == other.num_pixels ) && 
           ( self.file_size == other.file_size ) {
			return true;
		}
		
		return false;
    }
}

//Open an image from the specific path
//Tries to guess the format if it's not known
fn load_image_from_file( image_path: &str  ) -> std::result::Result<DynamicImage, MyImageError> {
	
	
	let img = match Reader::open(image_path) {
		Ok(image) => image,
		Err(_) => {
			return Err(MyImageError::FileError(format!("Error: Failed to read image file: {}", image_path).to_string()));
		},
	};
	
	let format_guessed = match img.with_guessed_format() {
		Ok( format_guessed ) => format_guessed,
		Err(_) => {
				return Err(MyImageError::DecodeFail(format!("Error: Failed to identify image file format {}", image_path).to_string()));
		}
	};
	
	let decoded_img = match format_guessed.decode() {
		Ok( decoded_img ) => decoded_img,
		Err(_) => {
				return Err( MyImageError::DecodeFail(format!("Error: Failed to correctly decode image: {}", image_path).to_string()) );
		}
	};
	
	return Ok(decoded_img);
}



impl ImageHashAV {
		
	pub const DEFAULT_COLOUR_DIFF_THRESHOLD: u64 = 256;	//Default colour difference threshold under which two images are declared dupes
	pub const DEFAULT_STD_DEV_THRESHOLD : f32 = 3.0;	//Default colour variation threshold under which de-duplication is not attempted
	pub const DEFAULT_ALG_FLIP_THRESHOLD : u64 = 20000; //Number of files at which we flip to the less accurate but faster algorithm
		
	pub fn new(fpath : &str) -> Result<ImageHashAV,MyImageError> {
		let mut object = ImageHashAV {	dupe_group: 0, grey_hash: 0, low_res: [0;192], 
						width: 0, height: 0, num_pixels: 0, std_dev: 0f32, 
						file_size: 0, fpath: "".to_string() };
		match object.calc_image_hash( &fpath ) {
			Some(e) => return Err(e),
			None => return Ok(object),
		}
	}
	
	//Check if two aspect ratios are within 2% of each other
	pub fn has_similar_aspect_ratio( &self, comp: &ImageHashAV ) -> bool {
		let aspect_ratio_a : f32 = self.width as f32 / self.height as f32;
		let aspect_ratio_b : f32 = comp.width as f32 / comp.height as f32;
		
		let aspect_ratio_a_high = aspect_ratio_a * 1.02;
		let aspect_ratio_a_low = aspect_ratio_a - (aspect_ratio_a * 0.02);
		
		if aspect_ratio_b <= aspect_ratio_a_high && aspect_ratio_b >= aspect_ratio_a_low {
			return true;
		}
		
		return false;
	}
	
	//Difference between the low_res version of this and another imagehash
	pub fn diff_colour( &self, comp: &ImageHashAV ) -> u64{
		
		let mut diff: u64 = 0;
		
		for i in 0..64 {
			let rdiff : u32 = (comp.low_res[(i*3)] as i32 - self.low_res[(i*3)] as i32).abs() as u32;
			let gdiff : u32 = (comp.low_res[(i*3)+1] as i32 - self.low_res[(i*3)+1] as i32).abs() as u32;
			let bdiff : u32 = (comp.low_res[(i*3)+2] as i32 - self.low_res[(i*3)+2] as i32).abs() as u32;
			
			diff += ( rdiff + gdiff + bdiff ) as u64;
		}
		
		return diff;
		
	}
	
	//Test if teo images are duplicates of each other
	pub fn is_dupe ( &self, other : &ImageHashAV, config: &ConfigOptions ) -> bool {
		//Excludes dark images with little variation which are difficult to dedupe correctly
		if self.std_dev > config.std_dev_threshold && other.std_dev > config.std_dev_threshold {	
			//Checks the images have a similar aspect ratio	
			if self.has_similar_aspect_ratio( &other ) {
				//Checks the colour differences are similar
				if self.diff_colour( &other ) <= config.colour_difference_threshold {
					return true;
				}
			}
		}
		
		return false;
	}
	
	//For each colour channel calculate the stdv of the pixels values and then take the average of the colour channels
	pub fn calc_std_dev_colour_hash ( &mut self ) {
		
		let mut r_pixel_av : f32 = 0.0;
		let mut g_pixel_av : f32 = 0.0;
		let mut b_pixel_av : f32 = 0.0;
		let mut r_square_total : f32 = 0.0;
		let mut g_square_total : f32 = 0.0;
		let mut b_square_total : f32 = 0.0;
		
		for i in 0..64 {
			r_pixel_av += self.low_res[(i*3)] as f32;
			g_pixel_av += self.low_res[(i*3)+1] as f32;
			b_pixel_av += self.low_res[(i*3)+2] as f32;
		}
		r_pixel_av /= 64.0;
		g_pixel_av /= 64.0;
		b_pixel_av /= 64.0;
		
		for i in 0..64 {
			r_square_total += ( (self.low_res[(i*3)] as f32) - r_pixel_av ).powf(2f32);
			g_square_total += ( (self.low_res[(i*3)+1] as f32) - g_pixel_av ).powf(2f32);
			b_square_total += ( (self.low_res[(i*3)+2] as f32) - b_pixel_av ).powf(2f32);
		}
		r_square_total /= 64.0;
		g_square_total /= 64.0;
		b_square_total /= 64.0;
		
		//Return average std_dev in the colours
		self.std_dev = (r_square_total.sqrt() + g_square_total.sqrt() + b_square_total.sqrt())/3.0;
		
	}
	
	pub fn calc_image_hash(&mut self, fpath: &str ) -> Option<MyImageError> {
		   
		match load_image_from_file( fpath ) {
			Ok(img) => {
				let (width, height) = img.dimensions();
				if width < 16 || height < 16 {
					return Some( MyImageError::ImageTooSmall(format!("Warning: Image too small to deduplicate: {}", fpath).to_string()) );
				}
		
				self.width = width;
				self.height = height;
				self.num_pixels = (width as u64)*(height as u64);
				self.fpath = fpath.to_string();
		
				//Get the file size as a tie breaker if image dimensions are the same
				match fs::metadata(fpath) {
					Ok(md)=> {
						self.file_size = md.len();
					}
					Err(_)=> {
						return Some(MyImageError::FileError(format!("Error: Failed to get size of: {}", fpath).to_string()));
					}
				}
		
								
				//Seems to work best with Gaussian, although it's the slowest
				let scaled = img.resize_exact(8,8,FilterType::Gaussian);
		
				let (width, height) = scaled.dimensions();
				if width != 8 || height != 8 {
					return Some( MyImageError::DecodeFail(format!("Error: Failed to resize image correctly: {}", fpath).to_string()) );
				}

				let gs = scaled.grayscale( );
		
				let mut num_pixels = 0;
				let mut total: u64 = 0;
				for pixel in gs.pixels() {
					let p: u64 = ((pixel.2).0)[0].into();
					total += p;
					num_pixels+=1;
				}
				let average: f32 = (total as f32)/ (num_pixels as f32);
		
				let mut hash_val: u64 = 0;
				let mut this_bit: u64 = 0;
		
				for pixel in gs.pixels() {
					let p: f32 = ((pixel.2).0)[0].into();
					if p >= average {
						hash_val = (((1 as u64) << this_bit ) as u64) | hash_val;
					}
					this_bit+=1;
				}				
		
				//Add the pixels of the low res original image into the struct
				let mut pnum : usize = 0;
				for pixel in scaled.pixels() {
					self.low_res[(pnum*3)] = ((pixel.2)[0]).into();
					self.low_res[((pnum*3)+1)] = ((pixel.2)[1]).into();
					self.low_res[(pnum*3)+2] = ((pixel.2)[2]).into();
					pnum+=1;
				}
		
				self.dupe_group = hash_val;
				self.grey_hash = hash_val;
				self.calc_std_dev_colour_hash();

				return None;
			},
			Err(e) => {
				return Some(e);
			}	
		}
	}

}

#[cfg(test)]
mod tests {
	extern crate glob;
	use super::*;
	use glob::glob;

	//Helper function to report the number of bits the same between two 64 bit values
    	fn calc_hamming_distance( a: u64, b: u64) -> u8 {
		let mut bits_similar : u8 = 0;
		for i in 0..64 {
			if (a & (1u64 << i)) == (b & (1u64 << i)) {
				bits_similar+=1;
			}
		}
		return bits_similar;
	}

	//Test an image is read and metadata extracted correctly
	#[test]
	fn test_image_read() {
		let result = ImageHashAV::new( "unit_test_images/bridge1_best.jpg" ).unwrap();
		assert_eq!(768,result.width,"Width OK");
		assert_eq!(576,result.height,"Height OK");
		assert_eq!(576*768,result.num_pixels,"NUm pixels OK");
	}
    
	//Test that images that should be duplicates of each other are correctly identified
	#[test]
	fn test_image_duplicates() {
		let mut image_paths = Vec::new();
	
		//Get a list of all the images in the unit_test_images directory.
		for entry in glob("unit_test_images/*").expect("Failed to read glob pattern") {
			let path = entry.unwrap().display().to_string();
			//Only list files that contain best and duplicate
			if path.contains("_best.") || path.contains("_duplicate_") {
				image_paths.push( path );
			}
		
		}
		//Sort the list such that each image with the name "best" should now be followed by exactly 2 duplicates.
		image_paths.sort();
		assert!(image_paths.len() >=3 , "3 or more image paths");
		assert!(image_paths.len() % 3 == 0, "Image paths divides by 3 (best + 2 duplicates per image)");
	
		//Check the best image matches the two duplicates
		for i in 0..(image_paths.len()/3) {
			let result = ImageHashAV::new( &image_paths[i*3] ).unwrap();
			let dupe1 = ImageHashAV::new( &image_paths[(i*3)+1] ).unwrap();
			let dupe2 = ImageHashAV::new( &image_paths[(i*3)+2] ).unwrap();
		
			//Check the duplicates match the best versions within a hamming distance of 1 bit (max 64 bits can be similar)
			assert!( calc_hamming_distance(result.dupe_group, dupe1.dupe_group) >= 63, "First duplicate grey hash matches" );
			assert!( calc_hamming_distance(result.dupe_group, dupe2.dupe_group) >= 63, "Second duplicate grey hash matches" );
		
			assert!( result.diff_colour( &dupe1 ) <= ImageHashAV::DEFAULT_COLOUR_DIFF_THRESHOLD, "First duplicate colours are similar" );
			assert!( result.diff_colour( &dupe2 ) <= ImageHashAV::DEFAULT_COLOUR_DIFF_THRESHOLD, "Second duplicate colours are similar" );
		}			
	}
    
	//Test that images which should not be duplicates of each other do not match
	#[test]
	fn test_image_uniques() {
		let mut image_paths = Vec::new();
		let mut image_hashes = Vec::new();
	
		//Get a list of images that should be unique
		for entry in glob("unit_test_images/*").expect("Failed to read glob pattern") {
			let path = entry.unwrap().display().to_string();
			if path.contains("_best.") {
				image_paths.push( path );
			}
		}
	
		for path in &image_paths {
			let result = ImageHashAV::new( &path ).unwrap();
			image_hashes.push( result );
		}
		
		//Test every image that should be unique against every other and match sure none match
		for i in 0..image_hashes.len() {			
			for j in (i+1)..image_hashes.len() {
				//Checks that either the images are at least 1 bit different on the grey hash
				//or else the colours do not match
				assert!( (calc_hamming_distance(image_hashes[j].dupe_group, image_hashes[i].dupe_group) <= 63) || 
						(image_hashes[j].diff_colour( &image_hashes[i] ) > ImageHashAV::DEFAULT_COLOUR_DIFF_THRESHOLD),
						"Images that should be unique don't match");
			}
		}
	}
}
