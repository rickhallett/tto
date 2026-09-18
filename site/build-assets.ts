// Keep the deployment self-contained: Vercel's Bun bundler includes JSON imports.
const text = (name: string) => Bun.file(new URL(name, import.meta.url)).text();
const data = JSON.stringify({
  html: await text('./index.html'),
  llms: await text('./llms.txt'),
  svg: await text('./logo.svg'),
  png: Buffer.from(await Bun.file(new URL('./logo.png', import.meta.url)).arrayBuffer()).toString('base64'),
});
const output = new URL('./assets.generated.json', import.meta.url);
if (process.argv.includes('--check')) {
  if (await Bun.file(output).text() !== data) throw new Error('Run bun site/build-assets.ts to refresh bundled assets');
} else {
  await Bun.write(output, data);
}
