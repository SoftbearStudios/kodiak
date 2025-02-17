all:
	convert -scale 50% -quality 90 -append *.png ranks.webp
	printf "data:image/webp;base64," > ranks.webp.url
	base64 -w 0 ranks.webp >> ranks.webp.url
