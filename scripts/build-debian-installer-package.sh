#!/usr/bin/bash

##############################################################################
# Builds a .deb installer for photodedupe for Debian Linux distros like Ubuntu
##############################################################################


PACKAGE_NAME="photodedupe_1.0.0_amd64"

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

PHOTO_DEDUPE_BINARY_PATH=${SCRIPT_DIR}/../target/release/photodedupe
MAN_PAGE_FILE="photodedupe.1"
MAN_PAGE_PATH=${SCRIPT_DIR}/../docs/${MAN_PAGE_FILE}
MAN_INSTALL_PATH=/usr/share/man/man1/

if [ -f ${PHOTO_DEDUPE_BINARY_PATH} ]; then

	mkdir /tmp/${PACKAGE_NAME}/
	mkdir /tmp/${PACKAGE_NAME}/DEBIAN/
	cp ${SCRIPT_DIR}/control /tmp/${PACKAGE_NAME}/DEBIAN/
	mkdir -p /tmp/${PACKAGE_NAME}/usr/bin/
	cp ${PHOTO_DEDUPE_BINARY_PATH} /tmp/${PACKAGE_NAME}/usr/bin/
	mkdir -p /tmp/${PACKAGE_NAME}/${MAN_INSTALL_PATH}
	cp ${MAN_PAGE_PATH} /tmp/${PACKAGE_NAME}/${MAN_INSTALL_PATH}
	gzip /tmp/${PACKAGE_NAME}/${MAN_INSTALL_PATH}/${MAN_PAGE_FILE}
	dpkg-deb --build --root-owner-group /tmp/${PACKAGE_NAME} .
	rm -Rf "/tmp/${PACKAGE_NAME}/"

else
	echo "Release version not built. Use cargo build --release. Binary expected at: ${PHOTO_DEDUPE_BINARY_PATH}"
fi


