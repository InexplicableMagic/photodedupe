[PhotoDedupe](https://github.com/InexplicableMagic/photodedupe) is a command line utility for finding duplicate photos regardless of image resolution, compression settings or file format. It compares photo content visually and does not rely on metadata to perform the de-duplication. Each set of duplicates is sorted by resolution to determine the best copy of each photo found. Photodedupe is multithreaded and can handle large sets of images.

## Downloads

Builds of the latest version for x86 Linux and Raspberry Pi are available for download from the [releases page](https://github.com/InexplicableMagic/photodedupe/releases).

Source code (Rust) is available from the [PhotoDedupe GitHub](https://github.com/InexplicableMagic/photodedupe)

## Example Usage

List images that have duplicates from these directories. The directories will be inspected recursively. Images will be sorted by resolution with the highest resolution first:

````
photodedupe dir_of_images_1/ dir_of_images_n/
````

List only duplicate photos:

````
photodedupe --duplicates dir_of_photos/ 
````

Show only unique photos:

````
photodedupe --uniques dir_of_photos/
````

Pipe in a list of file paths to images:

````
find photos/ -name '*.jpg' | photodedupe
````

Additional usage examples and more infomation can be found in the [ReadMe](https://github.com/InexplicableMagic/photodedupe/blob/main/README.md)
