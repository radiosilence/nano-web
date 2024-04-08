#!/bin/bash
cat <<EOF
{
   "Program":"${PKGNAME}_${PKGVERSION}/${PKGNAME}",
   "Args" : ["${PKGNAME}"],
   "Version":"${PKGVERSION}",
   "Arch": "${PKGARCH}"
}
EOF
