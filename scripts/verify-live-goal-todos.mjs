// Opt-in read-only live check; set YAHU_TEST_URL to the selected chat URL.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { readFile, mkdir } from 'node:fs/promises';
const target = process.env.YAHU_TEST_URL;
if (!target) throw new Error('YAHU_TEST_URL is required');
const url = new URL(target), base = url.origin;
const session = decodeURIComponent(url.hash.match(/^#\/chat\/([^/]+)/)?.[1] || '');
if (!session) throw new Error('an exact chat URL is required');
const out = process.env.YAHU_QA_OUT;
if (out) await mkdir(out, { recursive: true });
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const normalized = items => items.map(item => ({ id: item.id, content: item.content, status: item.status }));
const trimContent = content => { const chars = [...content.trim()]; return chars.length > 240 ? chars.slice(0, 240).join('') + '…' : chars.join(''); };
const assets = html => [...html.matchAll(/(?:src|href)="([^"]+)"/g)].map(x => x[1]).filter(x => x.startsWith('/assets/')).sort();
let browser, stage = 'login';
try {
  const raw = (await readFile(process.env.YAHU_ENV_FILE || '/etc/yahu.env', 'utf8')).match(/^HERMES_WEBUI_AUTH_KEY=(.*)$/m)?.[1]?.trim();
  assert(raw, 'login key unavailable');
  const key = /^(['"]).*\1$/.test(raw) ? raw.slice(1,-1) : raw;
  const login = await fetch(base+'/login', { method:'POST', redirect:'manual', body:new URLSearchParams({ password:key }) });
  const cookie = login.headers.get('set-cookie')?.split(';')[0];
  assert(login.status === 303 && cookie, 'login failed');
  const get = async path => {
    const response = await fetch(base + path, { headers: { Cookie: cookie } });
    assert(response.ok, 'live API read failed');
    return response.json();
  };
  stage = 'source-evidence';
  const canonical = await get(`/sessions/${encodeURIComponent(session)}/canonical`);
  const sourceSession = canonical.display_id || session;
  const payload = await get(`/hermes/api/sessions/${encodeURIComponent(sourceSession)}/messages?order=latest&limit=200`);
  const messages = payload.messages || payload.data;
  assert(Array.isArray(messages), 'unexpected message envelope');
  const source = messages.filter(m => m.role === 'tool' && ['todo', 'todo_list', 'functions.todo', 'functions.todo_list'].includes(m.tool_name)).sort((a,b) => a.timestamp - b.timestamp).flatMap(m => {
    try { const value = JSON.parse(m.content); return Array.isArray(value.todos) ? [{ timestamp:m.timestamp, todos:value.todos }] : []; } catch { return []; }
  }).pop();
  assert(source?.todos.length > 0, 'selected source window has no nonempty Todo result');
  const expected = source.todos.map(todo => ({ id:String(todo.id).trim(), content:trimContent(todo.content), status:todo.status }));
  stage = 'http-snapshot';
  const http = await get(`/chat/subagents/${encodeURIComponent(session)}/snapshot?before=${Date.now()/1000+1}`);
  assert(source.timestamp >= http.goal?.created_at, 'Todo evidence belongs to a different Goal generation');
  assert(JSON.stringify(normalized(http.goal?.todos || [])) === JSON.stringify(expected), 'HTTP Goal todos differ from source');
  const expectedAssets = assets(await readFile(new URL('../dist/index.html', import.meta.url), 'utf8'));
  browser = await chromium.launch({ headless:true, executablePath:process.env.CHROMIUM_EXECUTABLE || undefined, args:['--no-sandbox'] });
  for (const width of [1440,390]) {
    stage = `${width}:live-page`;
    const ctx = await browser.newContext({ viewport:{width,height:1000}, isMobile:width===390, hasTouch:width===390, serviceWorkers:'block' });
    const split=cookie.indexOf('=');
    await ctx.addCookies([{name:cookie.slice(0,split),value:cookie.slice(split+1),url:base,httpOnly:true,sameSite:'Lax'}]);
    const page=await ctx.newPage();
    let latest=null, errors=0, writes=0;
    page.on('pageerror',()=>errors++);
    page.on('websocket',ws=>{if(ws.url().includes('/chat/subagents/'))ws.on('framereceived',({payload})=>{try{const data=JSON.parse(String(payload));if(data.type==='subagents.snapshot')latest=data;}catch{}});});
    await page.route('**/*',route=>{if(route.request().method()!=='GET'){writes++;return route.abort();}return route.continue();});
    await page.goto(target, {waitUntil:'domcontentloaded'});
    await page.locator('.chat-scroll').waitFor();
    for(let attempt=0;attempt<80&&!latest;attempt++) await page.waitForTimeout(500);
    assert(latest?.session_id === session, 'selected-session live snapshot missing');
    assert(JSON.stringify(normalized(latest.goal?.todos || [])) === JSON.stringify(expected), 'WebSocket Goal todos differ from source');
    const goal=page.locator('.subagent-goal-panel');
    await goal.waitFor({state:'visible'});
    if(!await goal.evaluate(e=>e.open)) await goal.locator('summary').click();
    await page.waitForFunction(count => document.querySelectorAll('.subagent-goal-todos li').length === count, expected.length);
    const rows=await goal.locator('.subagent-goal-todos li').evaluateAll(rows=>rows.map(row=>({content:row.querySelector('span:last-child')?.textContent,status:['pending','in_progress','completed','cancelled'].find(status=>row.classList.contains(status))})));
    assert(rows.every((row,i)=>row.content===expected[i].content&&row.status===expected[i].status), 'rendered Todo content/status differs from source');
    const geometry=await goal.locator('.subagent-goal-todos').evaluate(list=>{
      const body=list.closest('.subagent-goal-body');
      const row=list.querySelector('li');
      const bounds=row.getBoundingClientRect(), area=body.getBoundingClientRect();
      return {display:getComputedStyle(list).display,firstVisible:bounds.bottom>area.top&&bounds.top<area.bottom,overflow:document.documentElement.scrollWidth>document.documentElement.clientWidth+1};
    });
    assert(geometry.display!=='none'&&geometry.firstVisible&&!geometry.overflow,'Todo list is hidden or outside the viewport');
    assert(JSON.stringify(assets(await page.content()))===JSON.stringify(expectedAssets),'deployed asset mismatch');
    assert(errors===0&&writes===0,'browser error or attempted write');
    if(out) await goal.screenshot({path:`${out}/live-goal-todos-${width}.png`});
    console.log(JSON.stringify({viewport:width,sourceTodos:expected.length,httpTodos:http.goal.todos.length,webSocketTodos:latest.goal.todos.length,visibleTodoRows:rows.length,contentAndStatusMatch:true,firstRowVisible:geometry.firstVisible,errors,writes}));
    await ctx.close();
  }
} catch(error) {
  console.error(JSON.stringify({stage,errorClass:error.constructor.name,assertion:error.constructor===Error?error.message:undefined}));
  process.exitCode=1;
} finally { await browser?.close(); }
