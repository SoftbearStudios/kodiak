#!/bin/sh
#
# release_game.sh
#
# SPDX-FileCopyrightText: 2024 Softbear, Inc.
# SPDX-License-Identifier: LGPL-3.0-or-later
#
# This script pushes the contents of the working directory to GitHub.

USAGE="usage: $0 repo repo_tag kodiak_tag action"
REPO_NAME="$1"
if [ -z "$REPO_NAME" ]; then
    echo $USAGE
    exit 1
fi
if [ ! -e .git ]
then
    echo "./.git not found: must run in root of $REPO_NAME repository"
    exit 1
fi
REPO_TAG="$2"
if [ -z "$REPO_TAG" ]; then
    echo $USAGE
    exit 1
fi
# If repo tag is in Cargo.toml: `cargo metadata --format-version=1 --no-deps | jq '.packages[0].version' | tr -d '"'`
KODIAK_TAG="$3"
if [ -z "$KODIAK_TAG" ]; then
    echo $USAGE
    exit 1
fi
ACTION="$4"
if grep "/${REPO_NAME}.git" .git/config > /dev/null
then
    echo "Repo name: $REPO_NAME"
else
    echo "must run in $REPO_NAME repository"
    exit 1
fi

TMPDIR=/tmp/github_release
if [ -d "$TMPDIR" ]; then
    echo "clear $TMPDIR"
    rm -rf $TMPDIR
fi
echo "Repo name: $REPO_NAME"
echo "Repo tag: $REPO_TAG"
echo "Kodiak tag: $KODIAK_TAG"
echo "Action: $ACTION"
echo "Checkout $REPO_NAME into $TMPDIR"
git clone git@github.com:softbearstudios/$REPO_NAME.git $TMPDIR
echo "Remove previous files (except for hidden files like .cargo, .git, .github, etc)"
rm -rf $TMPDIR/*
echo "Copy files into repo (but skip .cargo, .git, and .github)"
rsync -r . $TMPDIR --exclude engine --exclude .cargo --exclude .git --exclude .github --exclude .ssh --exclude server_fuzzer --exclude sprite_sheet_packer --exclude target --exclude .vscode
echo "Remove .bak files if any"
cd $TMPDIR; find . -name '*.bak' -exec rm {} \;
echo "Edit Cargo.toml files to have version $REPO_TAG"
cd $TMPDIR; find . -name Cargo.toml -exec sed -i -e "1,7s/^version\s*=\s*\"[^\"]*\"/version = \"${REPO_TAG}\"/" {} \;
echo "Edit Cargo.toml files to point to kodiak $KODIAK_TAG"
cd $TMPDIR; find . -name Cargo.toml -a -not -path './engine/*' -exec sed -i -e "/^kodiak_/s/path = \"[^\"]*\"/git = \"https:\/\/github.com\/softbearstudios\/kodiak\", tag=\"${KODIAK_TAG}\"/" {} \;
echo "Commit changes"
cd $TMPDIR; git add .
cd $TMPDIR; git commit -m $REPO_TAG
cd $TMPDIR; git tag $REPO_TAG
echo "Ready to push"
if [ "${ACTION}" = "push" ]; then
    echo "Push to github"
    cd $TMPDIR; git push; git push --tags
    rm -rf $TMPDIR
else
    echo "$REPO_NAME: edited version is in $TMPDIR"
fi
