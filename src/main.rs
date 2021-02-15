extern crate clap;
extern crate walkdir;
extern crate indicatif;

use std::path::Path;
use std::ffi::OsStr;
use std::io::{self, BufRead};
use clap::{Arg, App, ArgMatches, value_t};
use std::collections::HashSet;
use std::collections::HashMap;
use walkdir::{DirEntry, WalkDir};
use std::sync::mpsc::channel;
use threadpool::ThreadPool;
use indicatif::ProgressBar;


mod imagehash;
mod image_error;

fn main() {
	
	//Process command line arguments
	let matches = get_command_line_arguments();
	
	//Set the configuration options based on the command line
	match set_config_options( &matches ) {
		Ok(config) => {
			if !matches.is_present("debug") {
				//Gather the list of files to inspect
				match collate_file_list_any_source( &matches, &config ) {
					Some(dedup_file_list) => {
						
						//Calculate an image hash for each image and image statistics
						let results = run_image_hashing( dedup_file_list, &config );
				
						//Write out the list of duplicates per command line options
						output_results( results, &config );
					},
					None => {
						eprintln!("Didn't find any image files to test");
					},
				}
			
			}else{
				debug_mode( &matches, &config );
			}
		},
		Err(e) => eprintln!("{}",e),
	}
}

//Debug function to compare two images and return the internal statistics
fn debug_mode( matches: &ArgMatches, config : &imagehash::ConfigOptions ) {
	
		match matches.values_of("dir_or_file"){
			Some(mut paths) => {
				if paths.len() < 1 || paths.len() > 2 {
					eprintln!("Error: Debug mode requires either exactly 1 or 2 paths to images.");
				}else{
					match imagehash::ImageHashAV::new( paths.next().unwrap() )	{
						Ok(a) => {
							eprintln!("Pixel std_dev First:  {} ", a.std_dev );
							eprintln!("Grey Hash First:  {:x} ", a.grey_hash);
							
								
							if paths.len() > 0 {		
								match imagehash::ImageHashAV::new( paths.next().unwrap() ) {
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

fn get_default_config_options() -> imagehash::ConfigOptions {
	 return imagehash::ConfigOptions { colour_difference_threshold: imagehash::ImageHashAV::DEFAULT_COLOUR_DIFF_THRESHOLD, 
												std_dev_threshold : imagehash::ImageHashAV::DEFAULT_STD_DEV_THRESHOLD,
												alg_flip_threshold : imagehash::ImageHashAV::DEFAULT_ALG_FLIP_THRESHOLD,
												only_known_file_extensions : true,
												only_list_duplicates : false,
												only_list_uniques : false,
												list_all : false,
												num_threads : 4,
												 };
}


fn set_config_options( matches : &ArgMatches ) -> Result<imagehash::ConfigOptions,String> {
	
	let mut config : imagehash::ConfigOptions = get_default_config_options();
	
	config.only_list_duplicates = matches.is_present("duplicates");
	config.only_list_uniques = matches.is_present("uniques");
	config.list_all = matches.is_present("all");
	
	if matches.is_present("any-file") {
		config.only_known_file_extensions = false;
	}
	
	if matches.is_present("num_threads") {
		match value_t!(matches.value_of("num_threads"), u32) {
			Ok(num_threads) => {
				if num_threads < 1 {
					return Err("Number of threads must be greater than 0".to_string());
				}
				
				config.num_threads = num_threads;
			},
			Err(e) => {
				return Err(format!("For the number of threads option (-t): {}",e.to_string()));
			}
		}
	}
	
	return Ok(config);
	
}

fn get_command_line_arguments() ->  ArgMatches<'static> {
		let matches =  App::new("Photo Deduplicator")
		.arg(
			Arg::with_name("duplicates")
				.long("duplicates")
				.conflicts_with("uniques")
				.conflicts_with("all")
				.short("d")
				.required(false)
				.takes_value(false)
				.help("List only the detected duplicate images. Excludes the highest resolution version."),
		)
		.arg(
			Arg::with_name("uniques")
				.long("uniques")
				.conflicts_with("duplicates")
				.conflicts_with("all")
				.short("u")
				.required(false)
				.takes_value(false)
				.help("List only the best (highest resolution) version of each valid image without any duplicates."),
		)
		.arg(
			Arg::with_name("all")
				.long("all")
				.conflicts_with("duplicates")
				.conflicts_with("uniques")
				.short("a")
				.required(false)
				.takes_value(false)
				.help("List every unique image and the duplicates of each image grouped together."),
		).arg(
			Arg::with_name("any-file")
				.long("any-file")
				.short("y")
				.required(false)
				.takes_value(false)
				.help("Tests every file to see if it might be an image regardless of file extension. Allows image files with no extension."),
		).arg(
			Arg::with_name("num_threads")
				.long("threads")
				.short("t")
				.required(false)
				.takes_value(true)
				.help("Number of threads to use (default is 4)")
		
		).arg(
			Arg::with_name("debug")
				.long("debug")
				.short("g")
				.required(false)
				.takes_value(false)
				.help("Debug mode. Compare two files and explain why the files are either duplicates or unique."),
        ).arg(Arg::with_name("dir_or_file")
         .multiple(true))
        .get_matches();
        
        return matches;
}

fn collate_file_list_any_source( matches: &ArgMatches, config: &imagehash::ConfigOptions ) -> Option<HashSet<String>> {
	
	match gather_file_list_from_cmd_line( &matches ) {
		Some( st_files ) => {
			return Some(gather_file_list( &st_files, &config ));
		},
		None => {
			let st_files = gather_file_list_from_stdin( )?;
			return Some(gather_file_list( &st_files, &config ));
			
		},
	}
	
}

fn gather_file_list_from_stdin( ) -> Option<Vec<String>> {
	let mut path_list  : Vec<String> = Vec::new();
	
	let stdin = io::stdin();
    for line in stdin.lock().lines() {
			match line {
				Ok(line) => { 
					if !line.is_empty() {
						path_list.push( line )
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


//Read the command line arguments and generate a complete list of files to be traversed

fn gather_file_list_from_cmd_line( matches: &ArgMatches ) -> Option<Vec<String>> {
	let mut path_list  : Vec<String> = Vec::new();
	
	let iterator = matches.values_of("dir_or_file")?;
	
	for file_or_dir in iterator {
		path_list.push( file_or_dir.to_string() );
	}
	
	if path_list.len() < 1 {
		return None;
	}
	
	return Some( path_list );
	
}

//Only allows certain file extensions that may be images
//Unless the user has elected to allow all files to be tested
fn valid_file_extension( fpath: &Path, config: &imagehash::ConfigOptions ) -> bool {
	
	//ToDo: Move generation of this list outside function
   	//List of known image file extensions
	let known_extensions: HashSet<&str> = [ "jpg", "jpeg", "png", "tif", "tiff", "gif" ].iter().cloned().collect();
	
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

//Traverse any directories.
//Test the file paths found are valid and unique.
fn gather_file_list( path_list : &Vec<String>, config: &imagehash::ConfigOptions ) -> HashSet<String> {
  	   	
   	let mut dedup_file_list = HashSet::new();
   	    
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
	
	return dedup_file_list;
			
}

//Filter out invisible directories
fn dir_filter(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with("."))
         .unwrap_or(false)
}

fn run_image_hashing( dedup_file_list: HashSet<String>, config : &imagehash::ConfigOptions ) -> Vec<imagehash::ImageHashAV> {
	
	let mut image_hash_results: Vec<imagehash::ImageHashAV> = Vec::new();
	let mut error_list : Vec<image_error::MyImageError> = Vec::new();
	let mut num_threads : usize = config.num_threads as usize;
	let file_list_size: u64 = dedup_file_list.len() as u64;
	
	if file_list_size == 0 {
		eprintln!("No images found.");
		return image_hash_results;
	}
	
	if file_list_size < num_threads as u64 {
		num_threads = file_list_size as usize;
	} 
	
	
	//Calculate the image hashes on n threads
	let pool = ThreadPool::new(num_threads);
	
	let (tx, rx) = channel();
	for f in dedup_file_list {
		let tx = tx.clone();
		pool.execute(move|| {
			tx.send(imagehash::ImageHashAV::new( &f )).unwrap();
		});
	}
	drop(tx);

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
	progress_bar.finish();
		
	//Print any errors that ocurred while producing the hashes
	for e in error_list {
		eprintln!("{}", e.to_string());
	}
	
	//Use this version on small image sets
	if total_images_successfully_processed <= config.alg_flip_threshold {
		colour_n_square_check( &mut image_hash_results, &config );
	}else{
		eprintln!("Warn: Using less accurate comparison algorithm due to the number of images.");
		hamming_check( &mut image_hash_results, &config );
	}

	//Sort the grey hashes to group the matches with their putative duplicates
	image_hash_results.sort();

	return image_hash_results;
	
}

/* 
	 * Allow hamming distance of 1. Check if flipping a bit in the greyscale hash would cause a match against another hash.
	 * 
	 * This iterates through checking a 1 bit change in all 64-bits of each hash and testing it against all the hashes currently in the table.
	 * 
	 * I perceived this was faster than testing all images against all images as n*64 < n^2 where n > 64
	 * However on smaller image sets, less than about 10,000 images doing an n^2 colour check is fast enough
	 */
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

/*
 * Do an n^2 colour check - all against all check
 * Verifies that each image 
 */
 
fn colour_n_square_check( image_hash_results : &mut Vec<imagehash::ImageHashAV>, config : &imagehash::ConfigOptions ){
	
	for i in 0..image_hash_results.len(){
		image_hash_results[i].dupe_group = 0;
	}
	
	let mut dgroup : u64 = 1;
	
	let mut dupes_groups : HashMap<usize,u64> = HashMap::new();
	
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
	}
}

//Print the detected duplicates based on the command line options
fn output_results( image_hash_results : Vec<imagehash::ImageHashAV> , config : &imagehash::ConfigOptions  ){

	let mut last_unique_ih: imagehash::ImageHashAV = imagehash::ImageHashAV { dupe_group: 0, grey_hash: 0, low_res: [0;192], width: 0, height: 0, num_pixels: 0, std_dev : 0f32, file_size: 0, fpath: "".to_string() };
	let mut printed_uniq_header : bool = false;
	let mut not_first_it = false;
	
	let mut max_color_diff:u64 = 0;
	
	for imagehasher in image_hash_results {

		if not_first_it && imagehasher.dupe_group == last_unique_ih.dupe_group && 
			last_unique_ih.is_dupe( &imagehasher, &config )  {

			let cf : u64 = last_unique_ih.diff_colour( &imagehasher );
			if cf > max_color_diff {  max_color_diff = cf; }

			//eprintln!("ColourDiff:{} {} {}",last_unique_ih.diff_colour( &imagehasher ), last_unique_ih.fpath, imagehasher.fpath);
			
			if config.list_all {
				//println!("\tDuplicate: {:x} {}", imagehasher.dupe_group, imagehasher.fpath );
				println!("\tDuplicate: {}", imagehasher.fpath );
			}else if config.only_list_duplicates {
				println!("{}", imagehasher.fpath );
			}else if !config.only_list_uniques {
				if !printed_uniq_header {
					println!("Best({}x{}): {}", last_unique_ih.width, last_unique_ih.height, last_unique_ih.fpath );
					printed_uniq_header = true;
				}
				println!("\tDuplicate({}x{}): {}", imagehasher.width, imagehasher.height, imagehasher.fpath );
			}
		}else{
			
			printed_uniq_header = false;
			if config.only_list_uniques || config.list_all {
				//println!("s:{} {:x} {}", imagehasher.std_dev, imagehasher.dupe_group, imagehasher.fpath );
				println!("{}", imagehasher.fpath );
			}
						
			last_unique_ih = imagehasher;

		}
		
		not_first_it = true;
		
	}

	//eprintln!("Max Colour diff was: {}",max_color_diff );

}

#[cfg(test)]
mod tests {	
    use super::*;
    
    //Tests that the n square check identifies three images that should be duplicates as duplicates
    #[test]
    fn test_n_square_check() {
		let best = imagehash::ImageHashAV::new( "unit_test_images/cat1_best.jpg" ).unwrap();
		let dupe = imagehash::ImageHashAV::new( "unit_test_images/cat1_duplicate_1.jpg" ).unwrap();
		let dupe2 = imagehash::ImageHashAV::new( "unit_test_images/cat1_duplicate_2.jpg" ).unwrap();
		let mut images = vec![ dupe, best, dupe2 ];
		
		colour_n_square_check( &mut images, &get_default_config_options() );
		
		assert_eq!( images.len(), 3, "Should be three images" );
		assert_ne!( images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[2].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( images[0].dupe_group, images[1].dupe_group, "Images have same dupe group" );
		assert_eq!( images[0].dupe_group, images[2].dupe_group, "Images have same dupe group" );
	}
	
	
	#[test]
    fn test_hamming() {
		let best = imagehash::ImageHashAV::new( "unit_test_images/car1_best.jpg" ).unwrap();
		let dupe = imagehash::ImageHashAV::new( "unit_test_images/car1_duplicate_1.jpg" ).unwrap();
		let dupe2 = imagehash::ImageHashAV::new( "unit_test_images/car1_duplicate_2.jpg" ).unwrap();
		let mut images = vec![ dupe2, best, dupe ];
		
		hamming_check( &mut images, &get_default_config_options() );
		
		assert_eq!( images.len(), 3, "Should be three images" );
		assert_ne!( images[0].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[1].dupe_group, 0, "Dupe group is not zero" );
		assert_ne!( images[2].dupe_group, 0, "Dupe group is not zero" );
		assert_eq!( images[0].dupe_group, images[1].dupe_group, "Images have same dupe group" );
		assert_eq!( images[0].dupe_group, images[2].dupe_group, "Images have same dupe group" );
	}
	
}

