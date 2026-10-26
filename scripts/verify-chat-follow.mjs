// Browser-only fixtures: authenticates the deployed shell, mocks every data route.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { mkdir, readFile, writeFile } from 'node:fs/promises';
const base=process.env.YAHU_BASE_URL || 'http://127.0.0.1:9642';
const out=process.env.YAHU_QA_OUT;
if (out) await mkdir(out,{recursive:true});
let browser,stage='login';
const receipts=[];
try{
  const env=await readFile(process.env.YAHU_ENV_FILE || '/etc/yahu.env','utf8');
  const raw=env.match(/^HERMES_WEBUI_AUTH_KEY=(.*)$/m)?.[1]?.trim();
  if(!raw)throw new Error('missing key');
  const key=/^(['"]).*\1$/.test(raw)?raw.slice(1,-1):raw;
  const res=await fetch(base+'/login',{method:'POST',redirect:'manual',body:new URLSearchParams({password:key})});
  const cookie=res.headers.get('set-cookie')?.split(';')[0];
  if(res.status!==303||!cookie)throw new Error('login failed');
  const split=cookie.indexOf('=');
  browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_EXECUTABLE || undefined,args:['--no-sandbox']});
  const expectedHtml=await readFile(new URL('../dist/index.html',import.meta.url),'utf8');
  const assets=html=>[...html.matchAll(/(?:src|href)="([^"]+)"/g)].map(x=>x[1]).filter(x=>x.startsWith('/assets/')).sort();
  const expectedAssets=assets(expectedHtml);
  const timestamp=1800000000;
  for(const mobile of [false,true])for(const scenario of ['bottom','away','near-away','restored-away','returned-bottom']){
    stage=(mobile?'mobile':'desktop')+':'+scenario;
    const context=await browser.newContext({viewport:mobile?{width:390,height:844}:{width:1280,height:800},isMobile:mobile,hasTouch:mobile,deviceScaleFactor:2,serviceWorkers:'block'});
    await context.addCookies([{name:cookie.slice(0,split),value:cookie.slice(split+1),url:base,httpOnly:true,sameSite:'Lax'}]);
    await context.addInitScript((scenario)=>{
      if(scenario==='restored-away')localStorage.setItem('yahu.chat.view.v1',JSON.stringify({lastSessionId:'follow-fixture',positions:{'follow-fixture':900},anchors:{}}));
      localStorage.setItem('lang','en');localStorage.setItem('desktopCompactMessages','1');
      class FixtureEvents{constructor(url){if(String(url).startsWith('/chat/watch/'))window.__watch=this;}close(){}addEventListener(){}removeEventListener(){}}
      window.EventSource=FixtureEvents;window.WebSocket=FixtureEvents;
    },scenario);
    const page=await context.newPage();let errors=0,writes=0;
    page.on('pageerror',()=>errors++);
    const messages=Array.from({length:16},(_,i)=>[{id:String(i*2+1),role:'user',content:'Synthetic history question '+i,timestamp:timestamp+i*60},{id:String(i*2+2),role:'assistant',content:('Synthetic response paragraph '+i+'.\n\n').repeat(6),timestamp:timestamp+i*60+30}]).flat();
    const session={id:'follow-fixture',title:'Live scroll verification',source:'api',model:'test-model',message_count:messages.length,started_at:timestamp,last_active:timestamp+1000};
    await page.route('**/*',async route=>{
      const request=route.request(),url=new URL(request.url()),p=url.pathname;
      if(url.origin!==base)return route.abort();
      if(p==='/'||p.startsWith('/assets/')||p.startsWith('/icon')||p==='/manifest.json')return route.continue();
      if(request.method()!=='GET'){writes++;return route.abort();}
      let body={};
      if(p==='/models-cache')body={providers:[{slug:'test',name:'Test',models:['test-model']}]};
      if(p==='/sessions/search')body={data:[session]};
      if(p.startsWith('/chat/messages/'))body={data:messages,total:messages.length,has_older:false,has_newer:false};
      if(p.startsWith('/chat/user-nav/'))body={data:[],total:messages.length};
      if(p.endsWith('/canonical'))body={id:session.id,canonical_id:session.id};
      if(p.startsWith('/hermes/api/sessions/'))body={data:session};
      if(p.startsWith('/chat/context-window/'))body={used:5000,approximate:false};
      if(p.endsWith('/reasoning'))body={reasoning_effort:'medium'};
      if(p.endsWith('/status'))body={running:true};
      if(p==='/workspace/list')body={path:'',entries:[]};
      if(p.startsWith('/skills/'))body={data:[]};
      await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify(body)});
    });
    await page.goto(base+'/?follow-qa=1#/chat/follow-fixture');
    await page.waitForSelector('.msg-row.assistant');
    const deployedAssets=assets(await page.content());
    if(JSON.stringify(deployedAssets)!==JSON.stringify(expectedAssets))throw new Error('asset mismatch');
    await page.waitForFunction('!!window.__watch?.onmessage');
    await page.waitForTimeout(1300);
    if(scenario!=='restored-away')await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');e.scrollTop=e.scrollHeight;e.dispatchEvent(new Event('scroll',{bubbles:true}));});
    await page.waitForTimeout(100);
    if(scenario!=='bottom'&&scenario!=='restored-away'){
      await page.evaluate(distance=>{window.__awayDistance=distance;},scenario==='near-away'?24:180);
      await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');e.scrollTop-=window.__awayDistance;e.dispatchEvent(new Event('scroll',{bubbles:true}));});
      await page.waitForTimeout(100);
      if(scenario==='returned-bottom'){
        await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');e.scrollTop=e.scrollHeight-e.clientHeight-0.5;e.dispatchEvent(new Event('scroll',{bubbles:true}));});
        await page.waitForTimeout(100);
      }
    }
    const before=await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');return {top:e.scrollTop,distance:e.scrollHeight-e.clientHeight-e.scrollTop};});
    const emit=async message=>{await page.evaluate(msg=>window.__watch.onmessage({data:JSON.stringify(msg)}),message);await page.waitForTimeout(65);};
    await emit({id:'1001',role:'user',content:'New remote task',timestamp:timestamp+2000});
    const distances=[],positions=[];
    for(let n=1;n<=8;n++){
      await emit({id:'1002',role:'assistant',content:('Streaming synthetic content **with Markdown**.\n\n').repeat(n*3),pending:true,timestamp:timestamp+2001});
      const sample=await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');return {distance:e.scrollHeight-e.scrollTop-e.clientHeight,top:e.scrollTop};});
      distances.push(sample.distance);positions.push(sample.top);
    }
    await emit({id:'1002',role:'assistant',content:('Streaming synthetic content **with Markdown**.\n\n').repeat(24),pending:false,timestamp:timestamp+2001});
    await page.waitForTimeout(400);
    const after=await page.evaluate(()=>{const e=document.querySelector('.chat-scroll');return {top:e.scrollTop,distance:e.scrollHeight-e.clientHeight-e.scrollTop,bubble:document.querySelector('.new-messages-bubble')?.textContent||'',overflow:document.documentElement.scrollWidth>document.documentElement.clientWidth+1};});
    const passed=['away','near-away','restored-away'].includes(scenario)?Math.abs(after.top-before.top)<1&&positions.every(top=>Math.abs(top-before.top)<1)&&after.bubble.includes('2'):after.distance<=1&&!after.bubble&&distances.every(d=>d<=1);
    receipts.push({mobile,scenario,before,after,maxStreamDistance:Math.max(...distances),maxStreamMovement:Math.max(...positions.map(top=>Math.abs(top-before.top))),passed,errors,writes});
    if(out)await page.screenshot({path:out+`/yahu-follow-${mobile?'mobile':'desktop'}-${scenario}.png`});
    await context.close();
  }
  console.log(JSON.stringify(receipts,null,2));
  if(out)await writeFile(out+'/yahu-follow-verification.json',JSON.stringify(receipts,null,2));
  if(receipts.some(r=>!r.passed||r.errors||r.writes))process.exitCode=1;
}catch{console.error('QA failed at '+stage);process.exitCode=1;}finally{await browser?.close();}
