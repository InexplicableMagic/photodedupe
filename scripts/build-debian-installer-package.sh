#!/usr/bin/bash

##############################################################################
# Builds a .deb installer for photodedupe for Debian Linux distros like Ubuntu
##############################################################################


PACKAGE_NAME="photodedupe_1.0.1_amd64"

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)

TEMP_PATH="/tmp"
PHOTO_DEDUPE_BINARY_NAME=photodedupe
PHOTO_DEDUPE_BINARY_PATH=${SCRIPT_DIR}/../target/release/${PHOTO_DEDUPE_BINARY_NAME}
MAN_PAGE_FILE="photodedupe.1"
CHANGELOG_FILE="changelog"
COPYRIGHT_FILE="copyright"
MAN_PAGE_PATH=${SCRIPT_DIR}/../docs/man_page/${MAN_PAGE_FILE}
MAN_INSTALL_PATH=/usr/share/man/man1/

if command -v "dpkg-deb" &>/dev/null; then
	if [ -f ${PHOTO_DEDUPE_BINARY_PATH} ]; then

		mkdir "${TEMP_PATH}/${PACKAGE_NAME}/"
		mkdir "${TEMP_PATH}/${PACKAGE_NAME}/DEBIAN/"
		cp "${SCRIPT_DIR}/control" "${TEMP_PATH}/${PACKAGE_NAME}/DEBIAN/"
		mkdir -p ${TEMP_PATH}/${PACKAGE_NAME}/usr/bin/
		chmod 755 "${TEMP_PATH}/${PACKAGE_NAME}/usr/"
		chmod 755 "${TEMP_PATH}/${PACKAGE_NAME}/usr/bin/"
		cp "${PHOTO_DEDUPE_BINARY_PATH}" "${TEMP_PATH}/${PACKAGE_NAME}/usr/bin/"
		#Remove debugging symbols from binary
		strip "${TEMP_PATH}/${PACKAGE_NAME}/usr/bin/${PHOTO_DEDUPE_BINARY_NAME}"
		chmod 755 "${TEMP_PATH}/${PACKAGE_NAME}/usr/bin/${PHOTO_DEDUPE_BINARY_NAME}"
		mkdir -p "${TEMP_PATH}/${PACKAGE_NAME}/${MAN_INSTALL_PATH}"
		chmod 755 -R  "${TEMP_PATH}/${PACKAGE_NAME}/usr/share"
		chmod 755 -R  "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/man"
		chmod 755 -R  "${TEMP_PATH}/${PACKAGE_NAME}/${MAN_INSTALL_PATH}"
		cp "${MAN_PAGE_PATH}" "${TEMP_PATH}/${PACKAGE_NAME}/${MAN_INSTALL_PATH}"
		gzip -n -9 ${TEMP_PATH}/${PACKAGE_NAME}/${MAN_INSTALL_PATH}/${MAN_PAGE_FILE}
		chmod 644 "${TEMP_PATH}/${PACKAGE_NAME}/${MAN_INSTALL_PATH}/${MAN_PAGE_FILE}.gz"
		mkdir -p ${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/
		chmod 755 ${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/
		chmod 755 ${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/
		
		cp "${SCRIPT_DIR}/../docs/debian_specific/${CHANGELOG_FILE}" "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/"
		gzip -n -9 "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/${CHANGELOG_FILE}"
		chmod 644 "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/${CHANGELOG_FILE}.gz"
		
		cp "${SCRIPT_DIR}/../docs/debian_specific/${COPYRIGHT_FILE}" "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/"
		chmod 644 "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/${COPYRIGHT_FILE}"
		
		cp ../README.md "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/"
		gzip -n -9 "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/README.md"
		chmod 644 "${TEMP_PATH}/${PACKAGE_NAME}/usr/share/doc/${PHOTO_DEDUPE_BINARY_NAME}/README.md.gz"
		dpkg-deb --build --root-owner-group ${TEMP_PATH}/${PACKAGE_NAME} .
		rm -Rf "${TEMP_PATH}/${PACKAGE_NAME}/"
		
		#Runs lintian to validate the produced file and checks there are no warnings
		if command -v "lintian" &>/dev/null; then
			lint_output=$(lintian ./${PACKAGE_NAME}.deb)
			if [ -n "${lint_output}" ]; then
				echo "Lintian checks failed with messages:"
				echo "${lint_output}"
			else
				echo "Lintian checks all passed"
			fi
		else
			echo "Lintian not available on this system. Can't check intregrity of .deb file produced"
		fi
			

	else
		echo "Release version not built. Use cargo build --release. Binary expected at: ${PHOTO_DEDUPE_BINARY_PATH}"
	fi
else
	echo "dpkg-deb not available on this system. Can't build deb file."
fi

