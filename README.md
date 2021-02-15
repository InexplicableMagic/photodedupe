# photodedupe
Photodedupe is a command line utility for locating duplicate photos in a directory irrespective of image resolution, compression settings or file format. It sorts duplicates by resolution and identifies the highest resolution version present.

In the example below the image on the right has been exported with a higher level of compression than the image on the left. The two photos may appear visually identical but only about 18% of the pixels are actually the same, therefore traditional file de-duplication methods will not work. Photodedupe can identify both of these images as identical duplicates.

<img src="./unit_test_images/parrot1_best.jpg" width="200 height="266" /> <img src="./unit_test_images/parrot1_duplicate_2.jpg" width="200 height="266" />

## Usage

One or more directories can be supplied on the command line and photodedupe will recursively inspect them for images.

```
photodedupe dir_of_images/
```

An explicit list of files can be supplied

```
photodedupe image1.jpg image2.jpg image3.jpg
```

Or a list of files can be piped in:

```
find photos/ -name '*.jpg' | photodedupe
```

By default photodedupe will only inspect files with common image file extensions. JPEG, PNG, TIFF and GIF images are supported. However image file format can also be auto-detected. To inspect every file and determine if it may be an image, use the ```--any-file``` option.

## Output

The default output only lists images that have duplicates. The highest resolution version will be listed first as the "best" copy, followed by any lower resolution versions listed as duplicates.

```
Best(512x341): unit_test_images/cat2_best.jpg
	Duplicate(510x340): unit_test_images/cat2_duplicate_1.jpg
	Duplicate(100x67): unit_test_images/cat2_duplicate_2.png
```

To list every image file found regardless of whether it has a duplicate use the ```--all``` option.

To list only the highest resolution version of each image use the ```--uniques``` option.

To list only the lower resolution duplicate images, use the ```--duplicates``` option.

