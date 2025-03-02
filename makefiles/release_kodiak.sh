#!/bin/sh
#
# release_kodiak.sh
#
# SPDX-FileCopyrightText: 2024 Softbear, Inc.
# SPDX-License-Identifier: LGPL-3.0-or-later
#
# This script pushes the contents of the working directory to GitHub.

REPO_NAME="kodiak"
if [ ! -e .git ]
then
    echo "./.git not found: must run in root of $REPO_NAME repository"
    exit 1
fi

if grep "/${REPO_NAME}.git" .git/config > /dev/null
then
    echo "Repo name: $REPO_NAME"
else
    echo "must run in $REPO_NAME repository"
    exit 1
fi

USAGE="usage: $0 kodiak_tag action"
# If kodiak tag is in Cargo.toml: `cargo metadata --format-version=1 --no-deps | jq '.packages[0].version' | tr -d '"'`
KODIAK_TAG="$1"
if [ -z "$KODIAK_TAG" ]
then
    echo $USAGE
    exit 1
fi
ACTION="$2"
TMPDIR=/tmp/github_release
if [ -d "$TMPDIR" ];
then
    echo "clear $TMPDIR"
    rm -rf $TMPDIR
fi
echo "Kodiak tag: $KODIAK_TAG"
echo "Action: $ACTION"

echo "Checkout $REPO_NAME into $TMPDIR"
git clone git@github.com:softbearstudios/$REPO_NAME.git $TMPDIR
echo "Remove previous files (except for hidden files like .cargo, .git, .github, etc)"
rm -rf $TMPDIR/*
echo "Copy files into repo (but skip .cargo, .git, .github, and .gitlab-ci.yml)"
rsync -r . $TMPDIR --exclude archive --exclude .cargo --exclude Cargo.lock --exclude .git --exclude .github --exclude .gitlab-ci.yml --exclude manifest --exclude .ssh --exclude sprite_sheet_util --exclude target --exclude uploader --exclude .vscode
echo "Remove .bak files if any"
cd $TMPDIR; find . -name '*.bak' -exec rm {} \;
echo "Edit Cargo.toml files to have version $KODIAK_TAG"
cd $TMPDIR; find . -name Cargo.toml -exec sed -i -e "1,7s/^version\s*=\s*\"[^\"]*\"/version = \"${KODIAK_TAG}\"/" {} \;
echo "Commit changes"
cd $TMPDIR; git add .
cd $TMPDIR; git commit -m $KODIAK_TAG
cd $TMPDIR; git tag $KODIAK_TAG
echo "Ready to push"
if [ "${ACTION}" = "push" ]
then
    echo "Push to github"
    cd $TMPDIR; git push; git push --tags
    rm -rf $TMPDIR
else
    echo "$REPO_NAME: edited version is in $TMPDIR"
fi
