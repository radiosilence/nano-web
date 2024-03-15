#!/bin/bash
PKGNAME="nano-web"
cat <<EOF
{
   "Program":"${PKGNAME}_${PKGVERSION}/${PKGNAME}",
   "Args" : ["${PKGNAME}"],
   "Version":"${PKGVERSION}"
}
EOF
