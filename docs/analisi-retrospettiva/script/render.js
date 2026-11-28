const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1600, height: 1200 }, deviceScaleFactor: 1 });
  for (const f of process.argv.slice(2)) {
    const svg = fs.readFileSync(f, 'utf8');
    await p.setContent(`<html><body style="margin:0;background:#fff"><img id=i style="width:1600px" src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}"></body></html>`);
    await p.waitForTimeout(150);
    await (await p.$('#i')).screenshot({ path: f.replace(/\.svg$/, '.png') });
  }
  await b.close();
})();
