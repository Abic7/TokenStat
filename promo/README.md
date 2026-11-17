# TokenStat promo video

[Remotion](https://www.remotion.dev) source for `assets/demo/promo.mp4` and `promo.gif`.

```bash
npm install
npm run render
ffmpeg -i out/promo.mp4 -vf "fps=15,scale=800:-1:flags=lanczos,palettegen=max_colors=160:stats_mode=diff" pal.png
ffmpeg -i out/promo.mp4 -i pal.png -lavfi "fps=15,scale=800:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" ../assets/demo/promo.gif
```

The video is an animated illustration of the app, not a screen recording. Remotion has its own licence: free for individuals and small teams, a company licence for larger companies. See remotion.dev/license.
