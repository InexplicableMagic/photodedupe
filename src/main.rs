//!	# PhotoDedupe
//!	
//!	Photodedupe is a utility for identifying duplicate photos regardless of file name, image resolution, compression settings or file format. 
//!	It compares the image content visually and does not rely on any metadata to perform the de-duplication.
//!	
//!	`Usage`: photodedupe \<dir of images\>
//!	
//!	`Source`: [GitHub: InexplicableMagic/photodedupe](https://github.com/InexplicableMagic/photodedupe)
//!
//!	`License`: [MIT](https://mit-license.org/)
//!
//!	`Author` : LJ Bubb	


extern crate clap;
extern crate walkdir;
extern crate indicatif;

use std::path::Path;
use std::ffi::OsStr;
use std::io::{self, BufRead};
use clap::Parser;
use std::collections::HashSet;
use std::collections::HashMap;
use walkdir::{DirEntry, WalkDir};
use std::sync::mpsc::channel;
use threadpool::ThreadPool;
use indicatif::ProgressBar;

mod imagehash;
mod image_error;

/// PhotoDedupe: A utility for detecting duplicate photos in a collection of images
#[derive(Parser, Debug)]
#[command(version="1.0.0")]
struct Args {
    
    /// List only the detected duplicate images. Excludes the highest resolution version of each image. Excludes unique images.
    #[arg(short, long,  required = false, conflicts_with_all = &["uniques", "all"]) ]
    duplicates: bool,
    
    /// List only the best (highest resolution) version of each valid image without listing any duplicates.
    #[arg(short, long, required = false, conflicts_with_all = &["duplicates", "all"]) ]
    uniques: bool,
    
    /// By default photodedupe lists only images that have duplicates. This option causes all valid image files to be listed (except those below the minimum resolution if --min-resolution is used) regardless of whether the file has a duplicate.
    #[arg(short, long, required = false, conflicts_with_all = &["uniques", "duplicates"]) ]
    all: bool,
    
    /// Compares a directory of new images (supplied as the parameter to --compare) with one or more directories comprising an existing image collection (supplied as arguments). Tests whether each of the new images are duplicates of the existing image collection or unique depending on use of either the --duplicates or --uniques options respectively. When used with --duplicates, new images are classified as unique when of higher resolution than the version in the existing image collection. To mark similar images as duplicates in all circumstances (irrespective of resolution), additionally apply the --ignore-resolution option.
    #[arg(short, long="compare", required = false, value_name="directory of new images")]
    compare_dir: Option<String>,
    
    /// When using --compare always mark duplicates even the new image is better quality. Do not mark as unique even if better quality.
    #[arg(long = "ignore-resolution", required = false, requires="compare_dir" ) ]
    always_mark_duplicates: bool,
    
    /// Ignore all images of less than the specified resolution e.g. --min-resolution 300x200 will ignore images if either the width is less than 300 pixels or the height is less than 200 pixels.
    #[arg(long="min-resolution", required=false, value_name="WidthxHeight") ]
    ignore_low_res: Option<String>,
    
    /// Tests every file to see if it might be an image regardless of file extension. Also allows image files with no extension. The default behaviour is to only test files with common image filename extensions which are jpg,jpeg,png,tif,tiff,gif and webp.
    #[arg(short = 'y', long, required=false) ]
    any_file: bool,
    
    /// Only use the colour difference algorithm. This is more accurate but does not perform well with large numbers of images. This algorithm is used by default with 50,000 or fewer images.
    #[arg(long, required = false) ]
    force_colour_diff_only: bool,
    
    /// Number of CPU threads to use (default is 4). Higher number improves performance if more than 4 CPU threads are available.
    #[arg(short = 't', long = "threads", required=false, value_name="number of threads") ]
    num_threads: Option<u32>,
    
    /// Colour difference threshold. Higher value means more likely to consider images duplicates (Min:0,Max:49000,Default:256)
    #[arg(long, required=false, name="colour-diff-threshold", value_name="threshold" ) ]
    colour_diff_threshold: Option<u32>,
    
    /// Expects either one or two image file arguments. Where one file is supplied, prints statistics about the file. Where two are supplied prints statistics and information about the differences found between the files.
    #[arg(short = 'g', long, required = false, conflicts_with_all = &["uniques", "duplicates", "all", "compare_dir"]) ]
    debug: bool,
    
    #[arg(name = "Files/Directories", required = false)]
    dir_or_file: Option<Vec<String>>
}

fn main() {
	
	//Process command line arguments
	let matches = Args::parse();
	
	//Set the configuration options based on the command line
	match set_config_options( &matches ) {
		Ok(config) => {
			if !matches.debug {
				//Gather the list of files to inspect
				match collate_file_list_any_source( &matches, &config ) {
					Some(mut dedup_file_list) => {
						
						//Add in the images from the comparison directory
						if config.am_comparing {
							let mut path_list  : Vec<String> = Vec::new();
							path_list.push(config.compare_dir.clone());
							let compare_flist = gather_file_list( &path_list, &config, true );
							dedup_file_list.extend( compare_flist );
						}

						//Calculate an image hash for each image and image statistics
						let results = run_image_hashing( dedup_file_list, &config );
						
						if results.len() > 0 {
							//Write out the list of duplicates per command line options
							output_results( results, &config );
						}
						
					},
					None => {
						eprintln!("Error: Didn't find any image files to test");
					},
				}
			
			}else{
				debug_mode( &matches, &config );
			}
		},
		Err(e) => eprintln!("{}",e),
	}
}

/// Debug function to print internal statistics for an image. If two images are supplied, also compares them.
fn debug_mode( matches: &Args, config : &imagehash::ConfigOptions ) {
	
		match &matches.dir_or_file{
			Some(ref paths) => {
				if paths.len() < 1 || paths.len() > 2 {
					eprintln!("Error: Debug mode requires either exactly 1 or 2 paths to images.");
				}else{
					match imagehash::ImageHashAV::new( &imagehash::ImagePath{ fpath: paths.first().unwrap().to_string(), is_compare_dir: false, always_mark_dupe_compare: false }, config.min_width, config.min_height  )	{
						Ok(a) => {
							eprintln!("Pixel std_dev First:  {} ", a.std_dev );
							eprintln!("Grey Hash First:  {:x} ", a.grey_hash);
							
								
							if paths.len() > 1 {		
								match imagehash::ImageHashAV::new( &imagehash::ImagePath{ fpath: paths.get(1).unwrap().to_string(), is_compare_dir: false, always_mark_dupe_compare: false }, config.min_width, config.min_height ) {
									Ok(b) => {
										eprintln!("Grey Hash Second: {:x} ", b.grey_hash);
										eprintln!("Are grey hashes identical?: {}", (b.grey_hash == a.grey_hash) );
										eprintln!("Pixel std_dev Second: {} ", b.std_dev );
										eprintln!("Pixel colour difference: {} ", a.diff_colour( &b ));
										eprintln!("Are apect ratios similar?: {:?} ", a.has_similar_aspect_ratio( &b ));
										eprintln!("Are both images duplicates?:  {} ", b.is_dupe(&a, &config) );
									},
									Err(e) => {
										eprintln!("{}", e);
									},
								}
							}
						},
						Err(e) => {
							eprintln!("{}", e);
						},
					}
				}
			},
			None => {
				eprintln!("Error: Debug mode requires either exactly 1 or 2 paths to images.");
			}
		
		}
		  
}

/// Returns a command line configuration options object with a set of reasonable defaults configured
fn get_default_config_options() -> imagehash::ConfigOptions {
	return imagehash::ConfigOptions { colour_difference_threshold: imagehash::ImageHashAV::DEFAULT_COLOUR_DIFF_THRESHOLD, 
												std_dev_threshold : imagehash::ImageHashAV::DEFAULT_STD_DEV_THRESHOLD,
												alg_flip_threshold : imagehash::ImageHashAV::DEFAULT_ALG_FLIP_THRESHOLD,
												alg_colour_diff_only : false,
												only_known_file_extensions : true,
												only_list_duplicates : false,
												only_list_uniques : false,
												list_all : false,
												num_threads : 4,
												compare_dir : "".to_string(),
												am_comparing : false,
												always_mark_duplicates : false,
												min_width: 0,
												min_height : 0,							
									};
}

/// Converts configuration options set on the command line with the Clap module into the internal configuration options object
fn set_config_options( matches : &Args ) -> Result<imagehash::ConfigOptions,String> {
	
	let mut config : imagehash::ConfigOptions = get_default_config_options();
	
	config.only_list_duplicates = matches.duplicates;
	config.only_list_uniques = matches.uniques;
	config.list_all = matches.all;
	config.alg_colour_diff_only = matches.force_colour_diff_only;
	config.always_mark_duplicates = matches.always_mark_duplicates;
	
	if matches.any_file {
		config.only_known_file_extensions = false;
	}

	match matches.num_threads {
		Some(num_threads) => {
			if num_threads < 1 {
				return Err("Number of threads must be greater than 0".to_string());
			}
			
			config.num_threads = num_threads;
		}, None => {}
	}
	
	match matches.colour_diff_threshold {
		Some(colour_diff_threshold) =>  {
			if colour_diff_threshold > 49000 {
				return Err("colour_diff_threshold must be between 0 - 49000 inclusive.".to_string());
			}
			config.colour_difference_threshold = colour_diff_threshold as u64;
		}, None => {}
	}


	match &matches.compare_dir {
		Some(ref c_dir) => {
			let dir_test = Path::new(&c_dir);
			if dir_test.is_dir() || dir_test.is_file() {
				config.compare_dir = c_dir.to_string();
				config.am_comparing  = true;
			}else{
				return Err(format!("Option to --compare \"{}\" is not a valid directory or file.", c_dir));
			}
		}, None => {}	//If the string is missing it should be caught by clap
	}
	
	match &matches.ignore_low_res {
		Some(ref width_height) => {
			if let Some((width,height)) = extract_width_and_height( width_height ) {
				 if width < 16 || height < 16 {
				 	return Err("Images with width or height of less than 16 pixels are always ignored.".to_string());
				 }
				 config.min_width = width;
				 config.min_height = height;

			}else{
				return Err("Paramater passed to --min-resolution option is incorrectly formatted. Should be widthxheight e.g. 100x100.".to_string());
			}
		}, None =>{ } //If the string is missing it should be caught by clap
	}
	
	return Ok(config);
	
}

/// Given a string of the format "widthxheight", extract the width and height as integers
fn extract_width_and_height(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return None;
    }
    let width = parts[0].parse::<u32>();
    let height = parts[1].parse::<u32>();
    match (width, height) {
        (Ok(w), Ok(h)) => Some((w, h)),
        _ => None,
    }
}

/// Determines a list of image file paths that the utility is going to compare
fn collate_file_list_any_source( matches: &Args, config: &imagehash::ConfigOptions ) -> Option<Vec<imagehash::ImagePath>> {
	
	match gather_file_list_from_cmd_line( &matches ) {
		Some( st_files ) => {
			return Some(gather_file_list( &st_files, &config, false ));
		},
		None => {
			let st_files = gather_file_list_from_stdin( )?;
			return Some(gather_file_list( &st_files, &config, false ));
			
		},
	}
	
}

/// Gather a list of image file paths passed in on stdin
fn gather_file_list_from_stdin( ) -> Option<Vec<String>> {
	let mut path_list  : Vec<String> = Vec::new();
	
	let stdin = io::stdin();
    	for line in stdin.lock().lines() {
			match line {
				Ok(line) => {
					let line_trimmed  = line.trim();
					if !line_trimmed.is_empty() {
						//Handle Windows style linefeeds
						let crlf_cleaned_line = line_trimmed.trim_end_matches('\r');
						path_list.push( crlf_cleaned_line.to_string() )
					}
				},
				Err(e) => {
					eprintln!("Error reading from stdin: {}",e.to_string());
				}
			}
			
	}
	
	if path_list.len() < 1{
		return None;
	}
	
	return Some(path_list);
}


/// Read the command line arguments and generate a complete list of files to be traversed
fn gather_file_list_from_cmd_line( matches: &Args ) -> Option<Vec<String>> {
	let mut path_list  : Vec<String> = Vec::new();
	
	match &matches.dir_or_file {
		
		Some(ref paths) => {
			for file_or_dir in paths {
				path_list.push( file_or_dir.to_string() );
			}
			
			if path_list.len() < 1 {
				return None;
			}
		}, None => {
			return None;
		}
	
	}
	
	return Some( path_list );
	
}

/// Determines if a specific file path has one of an allowed list of image file extensions
fn valid_file_extension( fpath: &Path, config: &imagehash::ConfigOptions ) -> bool {
	
   	//List of known image file extensions
	let known_extensions: HashSet<&str> = [ "jpg", "jpeg", "png", "tif", "tiff", "gif", "webp" ].iter().cloned().collect();
	
	//If any-file is not set, only tests a limited list of file extensions
	if config.only_known_file_extensions {
		match fpath.extension().and_then(OsStr::to_str) {
			Some(extension)=>{
				let ext_lower = extension.to_lowercase();
				if !known_extensions.contains(&ext_lower.as_str()) {
					return false;
				}
			},
			None => return false,
		}
	}
	
	return true;
	
}

/// Recusively inspects directories and extracts all of the files found
fn gather_file_list( path_list : &Vec<String>, config: &imagehash::ConfigOptions, am_comparing : bool ) -> Vec<imagehash::ImagePath> {
  	   	
   	let mut dedup_file_list = HashSet::new();
	let mut output_image_paths  : Vec<imagehash::ImagePath> = Vec::new();
   	    
	for file_or_dir in path_list {
		let fod_test = Path::new(file_or_dir);
		if fod_test.exists() {
			if fod_test.is_file() {
				if valid_file_extension( &fod_test, &config ) {
					dedup_file_list.insert( file_or_dir.to_string() );
				}
			}
			//If the command line argument is a directory, then recursively traverse it
			if fod_test.is_dir() {
				let recurse_dir = WalkDir::new(file_or_dir).into_iter();
				for entry in recurse_dir.filter_entry(|e| !dir_filter(e)) {
					let entry_u = entry.unwrap();
					let path = entry_u.path();
					if path.exists() && path.is_file() {
						if valid_file_extension( &path, &config ) {
							dedup_file_list.insert( path.to_str().unwrap().to_owned() );
						}
						
					}
				}
			}
		}else{
			eprintln!("ERROR: Failed to read: {}", file_or_dir);
		}
	}
	
	for path in dedup_file_list {
		let mut always_mark : bool = false;
		if am_comparing {
			always_mark = config.always_mark_duplicates;
		}
		output_image_paths.push( imagehash::ImagePath { fpath: path, is_compare_dir: am_comparing, always_mark_dupe_compare: always_mark } );
	}

	return output_image_paths;
			
}

/// Filter to ignore invisible files that start with a dot
fn dir_filter(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with("."))
         .unwrap_or(false)
}

/// Accepts a list of file paths and returns an ordered list of metadata with possible (but not confirmed) duplicates grouped together
fn run_image_hashing( dedup_file_list: Vec<imagehash::ImagePath>, config : &imagehash::ConfigOptions ) -> Vec<imagehash::ImageHashAV> {
	
	let mut image_hash_results: Vec<imagehash::ImageHashAV> = Vec::new();
	let mut error_list : Vec<image_error::MyImageError> = Vec::new();
	let mut num_threads : usize = config.num_threads as usize;
	let file_list_size: u64 = dedup_file_list.len() as u64;
	let min_w  = config.min_width;
	let min_h = config.min_height;
	
	if file_list_size == 0 {
		eprintln!("No images found.");
		return image_hash_results;
	}
	
	//If there are few images, use only one thread per image
	if file_list_size < num_threads as u64 {
		num_threads = file_list_size as usize;
	} 
	
	//Deduplication is a two step process:
	//In step one we gather statistics about the image files
	//In step two we then perform comparisons of the image statistics
	
	//Calculate the image hashes on n threads
	//The number of threads can be set using a command line option
	let pool = ThreadPool::new(num_threads);
	
	let (tx, rx) = channel();
	for f in dedup_file_list {
		let tx = tx.clone();
		pool.execute(move|| {
			tx.send(imagehash::ImageHashAV::new( &f, min_w, min_h )).unwrap();
		});
	}
	drop(tx);

	//Perform step one: gather statistics
	//Draw a progress bar for the user.
	let progress_bar = ProgressBar::new(file_list_size);
	
	let mut total_images_successfully_processed : u64 = 0;
	
	//Collate the output of the threads
	for t_result in rx.into_iter(){
		match t_result {
			Ok(img_result)=> {
				image_hash_results.push( img_result );
				total_images_successfully_processed +=1;
			}
			Err(e)=>{
				//Store the errors to print later, as printing them live disrupts the progress bar
				error_list.push( e )
			}	
		}
		progress_bar.inc(1);
		
	}
	progress_bar.finish_and_clear();
		
	//Print any errors that ocurred while producing the hashes
	for e in error_list {
		eprintln!("{}", e.to_string());
	}
	
	//Now move onto step two and compare the image statistics
	
	//Use this algorithm on small image sets - often a little more accurate but doesn't scale well
	if (total_images_successfully_processed <= config.alg_flip_threshold) || config.alg_colour_diff_only {
		colour_n_square_check( &mut image_hash_results, &config );
	}else{
		//Use this considerably faster algorithm on larger image sets. "Large" is defined by config.alg_flip_threshold
		eprintln!("Warn: Using less accurate comparison algorithm due to the number of images.");
		hamming_check( &mut image_hash_results, &config );
	}

	//Sort the grey hashes to group the matches with their putative duplicates
	image_hash_results.sort();

	return image_hash_results;
	
}

/// Determines if images might be duplicates using a method of checking hamming distances of perceptual hashes
///
/// Allow hamming distance of 1. Check if flipping a bit in the greyscale hash would cause a match against another hash.
/// 
/// This iterates through checking a 1 bit change in all 64-bits of each hash and testing it against all the hashes currently in the table.
/// 
/// I perceived this was faster than testing all images against all images as n*64 < n^2 where n > 64
/// However on smaller image sets, less than about 50,000 images doing an n^2 colour check is fast enough

fn hamming_check( image_hash_results : &mut Vec<imagehash::ImageHashAV>, config : &imagehash::ConfigOptions ){
	
	let mut all_hash_codes = HashMap::new();
	
	image_hash_results.sort_by(|a, b| b.num_pixels.cmp(&a.num_pixels));

	for imagehasher in image_hash_results.iter_mut() {
		if all_hash_codes.len() == 0 {
			all_hash_codes.insert( imagehasher.dupe_group, imagehasher );
		}else{
			if !all_hash_codes.contains_key( &imagehasher.dupe_group ){
				let test_hash = imagehasher.dupe_group;
				//Try every variation of the current hash with 1 bit flipped
				//Check if flipping one bit would cause a match
				let mut putative_match_hash : u64 = 0;
				let mut last_putative_size : u64 = 0;
				for n in 0..64 {
					let flipped_bit_hash = ((1 as u64) << n) ^ test_hash;
					if all_hash_codes.contains_key( &flipped_bit_hash ) {
						//Only accept the bit flip if the colour check also matches and the aspect ratios are similar
						if imagehasher.is_dupe( all_hash_codes.get( &flipped_bit_hash ).unwrap(), &config ) {					
							//Prefer the match with the largest number of pixels
							if imagehasher.num_pixels > last_putative_size{
								//Set the hash to be the same as the one with the flipped bit
								last_putative_size = all_hash_codes.get( &flipped_bit_hash ).unwrap().num_pixels;
								putative_match_hash = flipped_bit_hash
							}
						}
					}
				}
				
				if last_putative_size > 0 && putative_match_hash > 0 {
					imagehasher.dupe_group = putative_match_hash;
				}else{
					all_hash_codes.insert( test_hash, imagehasher );
				}
				
			}
			
		}
	}
}

/// Determine if images might be duplicates by using an n^2 scaling method (compares every image against every other) 
 
fn colour_n_square_check( image_hash_results : &mut Vec<imagehash::ImageHashAV>, config : &imagehash::ConfigOptions ){
	
	for i in 0..image_hash_results.len(){
		image_hash_results[i].dupe_group = 0;
	}
	
	let mut dgroup : u64 = 1;
	
	let mut dupes_groups : HashMap<usize,u64> = HashMap::new();
	
	//Display a 2nd progress bar as this can take a long time
	let progress_bar = ProgressBar::new(image_hash_results.len() as u64);
	
	for i in 0..image_hash_results.len() {
		
		for j in (i+1)..image_hash_results.len() {	
						
					if image_hash_results[i].is_dupe( &image_hash_results[j], &config ) {
						
						if dupes_groups.contains_key(&i) {
							let d : u64 = *dupes_groups.get( &i ).unwrap();
							image_hash_results[j].dupe_group = d;
							dupes_groups.insert( j, d );
						}else if dupes_groups.contains_key(&j) {
							let d : u64 = *dupes_groups.get( &j ).unwrap();
							image_hash_results[i].dupe_group = *dupes_groups.get( &j ).unwrap();
							dupes_groups.insert( i, d );
						}else{
							dupes_groups.insert( i, dgroup );
							image_hash_results[i].dupe_group  = dgroup;
							image_hash_results[j].dupe_group  = dgroup;
						}
						
					}	
			dgroup +=1;
		}

		progress_bar.inc(1)
	}
	
	progress_bar.finish_and_clear();
}

/// Print the detected duplicates based on preferneces specified in command line options
fn output_results( image_hash_results : Vec<imagehash::ImageHashAV> , config : &imagehash::ConfigOptions  ){

	let mut last_unique_ih: imagehash::ImageHashAV = imagehash::ImageHashAV { dupe_group: 0, grey_hash: 0, low_res: [0;192], width: 0, height: 0, num_pixels: 0, std_dev : 0f32, file_size: 0, image_path: imagehash::ImagePath{ fpath: "".to_string(), is_compare_dir: false, always_mark_dupe_compare: false } };
	let mut printed_uniq_header : bool = false;
	let mut not_first_it = false;
		
	let mut num_unique_images : u64 = 0;
	let mut num_dupe_images : u64 = 0;
	
	
	for imagehasher in image_hash_results {

		if not_first_it && imagehasher.dupe_group == last_unique_ih.dupe_group && 
			last_unique_ih.is_dupe( &imagehasher, &config )  {			
			if config.list_all {
				println!("\tDuplicate: {}", imagehasher.image_path.fpath );
			}else if config.only_list_duplicates {
				//If using --compare, only report the duplicate if it is in the comparison dir
				if (!config.am_comparing) || imagehasher.image_path.is_compare_dir {
					println!("{}", imagehasher.image_path.fpath );
				}
			}else if !config.only_list_uniques {
				//If using --compare, only report if the best or duplicate is in the comparison dir
				if (!config.am_comparing) || last_unique_ih.image_path.is_compare_dir || imagehasher.image_path.is_compare_dir {
					if !printed_uniq_header {
						println!("Best({}x{}): {}", last_unique_ih.width, last_unique_ih.height, last_unique_ih.image_path.fpath );
						printed_uniq_header = true;
					}
					println!("\tDuplicate({}x{}): {}", imagehasher.width, imagehasher.height, imagehasher.image_path.fpath );
				}
			}
			num_dupe_images+=1;
		}else{
			printed_uniq_header = false;
			if config.only_list_uniques || config.list_all {
				//If using --compare, only report the unique image if it is in the comparison dir
				if (!config.am_comparing) || imagehasher.image_path.is_compare_dir {
					println!("{}", imagehasher.image_path.fpath );
				}
			}
						
			last_unique_ih = imagehasher;
			num_unique_images+=1;
		}
		not_first_it = true;
	}
	
	if (!config.only_list_duplicates) && (!config.only_list_uniques) && (!config.list_all) && (!config.am_comparing) {
		eprintln!("Unique Images: {} Duplicates: {}", num_unique_images, num_dupe_images);
	}

}


#[cfg(test)]
mod tests {	
    use super::*;
    
	/// Tests that the n square check identifies three images that should be duplicates as duplicates
	#[test]
	fn test_n_square_check() {
		let best = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/cat1_best.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let dupe = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/cat1_duplicate_1.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let dupe2 = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/cat1_duplicate_2.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let mut images = vec![ dupe, best, dupe2 ];
		
		colour_n_square_check( &mut images, &get_default_config_options() );
		
		assert_eq!( images.len(), 3, "Should be three images" );
		assert_ne!( images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[2].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( images[0].dupe_group, images[1].dupe_group, "Images have same dupe group" );
		assert_eq!( images[0].dupe_group, images[2].dupe_group, "Images have same dupe group" );
	}
	
	/// Tests that when using the hamming method images are identified as duplicates
	#[test]
	fn test_hamming() {
		let best = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/car1_best.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let dupe = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/car1_duplicate_1.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let dupe2 = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/car1_duplicate_2.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let mut images = vec![ dupe2, best, dupe ];
		
		hamming_check( &mut images, &get_default_config_options() );
		
		assert_eq!( images.len(), 3, "Should be three images" );
		assert_ne!( images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[2].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( images[0].dupe_group, images[1].dupe_group, "Images have same dupe group" );
		assert_eq!( images[0].dupe_group, images[2].dupe_group, "Images have same dupe group" );
	}

	#[test]
	fn test_compare_option() {
		//Test the --compare option

		//Put the highest resolution image in the compare directory and used the --always-mark-duplicates option
		let best = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/car1_best.jpg".to_string(), is_compare_dir:true, always_mark_dupe_compare: true },0,0 ).unwrap();
		//Lower resolution image
		let dupe = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/car1_duplicate_1.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let mut images = vec![ best, dupe ];

		colour_n_square_check( &mut images, &get_default_config_options() );
		images.sort();

		//Test the images are actually identified as duplicates
		assert_eq!( images.len(), 2, "Should be two images" );
		assert_ne!( images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( images[0].dupe_group, images[1].dupe_group, "Images have same dupe group" );

		//Test that they are ordered such as the highest resolution image is lower down because it is in the comparison directory. This forces identification as a duplicate even though it is better quality
		assert_eq!( images[0].image_path.fpath, "unit_test_images/car1_duplicate_1.jpg", "Duplicate should be top of the list because not in the compare directory." );
		assert_eq!( images[1].image_path.fpath, "unit_test_images/car1_best.jpg", "Best image should be second on the list because is in the compare directory." );

		
		//Test that when images are identical the one in the compare directory should sort last when using -always-mark-duplicates option
		let t2_best = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/book1_best.jpg".to_string(), is_compare_dir:true, always_mark_dupe_compare: true },0,0 ).unwrap();
		let t2_dupe1 = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/book1_best.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let t2_dupe2 = imagehash::ImageHashAV::new( &imagehash::ImagePath { fpath: "unit_test_images/book1_best.jpg".to_string(), is_compare_dir:false, always_mark_dupe_compare: false },0,0 ).unwrap();
		let mut t2_images = vec![ t2_best, t2_dupe1, t2_dupe2 ];

		hamming_check( &mut t2_images, &get_default_config_options() );
		t2_images.sort();

		assert_eq!( t2_images.len(), 3, "Should be three images" );
		assert_ne!( t2_images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( t2_images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( t2_images[2].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( t2_images[0].dupe_group, t2_images[1].dupe_group, "Images have same dupe group" );
		assert_eq!( t2_images[0].dupe_group, t2_images[2].dupe_group, "Images have same dupe group" );
		assert_eq!( t2_images[0].image_path.is_compare_dir, false, "The 1st image is not in the compare directory" );
		assert_eq!( t2_images[1].image_path.is_compare_dir, false, "The 2nd image is not in the compare directory" );
		assert_eq!( t2_images[2].image_path.is_compare_dir, true, "The image in the compare directory is last in the sort group" );
	}
	
}

