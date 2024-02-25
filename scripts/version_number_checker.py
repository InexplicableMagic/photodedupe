#!/usr/bin/env python3

##########################################################################################
# Script to check the version number is in sync in all the various files where it appears
# when the version number of the package is updated.
#
# Supply the version number that should exist in all files as the first argument
#
# Usage: python3 version_number_checker.py <version>
#
##########################################################################################

import re
import sys
from pathlib import Path

#A file and a regex to extract the version number found in that file
version_locations = [
		# There's a version number in the help line in main.rs: #[command(version="1.0.1")]
		( '../src/main.rs', r'\#\[command\(version=\"(\d+\.\d+\.\d+)\"\)\]' ),
		# Version in the Cargo manifest: version = "1.0.1"
		( '../Cargo.toml', r'version\s{0,1}\=\s{0,1}\"(\d+\.\d+\.\d+)\"' ),
		# Version in the .deb control file: Version: 1.0.1
		( './control', r'Version\:\s{0,1}(\d+\.\d+\.\d+)' ),
		# Should be an entry against the current version in the Debian changelog file: photodedupe (1.0.1)
		( '../docs/debian_specific/changelog', r'photodedupe \((\d+\.\d+\.\d+)' ),
		# There's a version in the metadata of the source for the man page
		( '../docs/man_page/man_page_source.md', r'\% photodedupe\(1\) Version (\d+\.\d+\.\d+)' ),
		# Version in the man page
		( '../docs/man_page/photodedupe.1', r'\.TH \"photodedupe\" .*?Version (\d+\.\d+\.\d+)' ),
		
]

def check_file( version, fpath, regex ):
	pattern = re.compile(regex)

	with open(fpath, "r") as textfile:
		for line_number, line in enumerate(textfile):
			line_matches = pattern.match(line)
			if line_matches:
				if line_matches.group(1) == version:
					return True
					
					
	return False
			
if __name__ == "__main__":

	if len(sys.argv) > 1:
		version = sys.argv[1]

		for f, regex in version_locations:
			if Path(f).is_file():
				check_result = check_file(version, f, regex)
				if not check_result:
					print("Failed to find correct version number in:"+str(f), file=sys.stderr)
					sys.exit(1)
				else:
					print(str(f)+" ... ok",file=sys.stderr)
			else:
				print("Failed to find file:"+str(f), file=sys.stderr)
				sys.exit(1)
	else:
		print("First argument should be version number to be checked.", file=sys.stderr)
		sys.exit(1)
	
