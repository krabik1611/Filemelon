const fs = require('node:fs');
const ts = require('typescript');
const assert = require('node:assert/strict');
const test = require('node:test');
const source = ts.createSourceFile('app.component.ts', fs.readFileSync('src/app/app.component.ts', 'utf8'), ts.ScriptTarget.Latest, true);
const component = source.statements.find(node => ts.isClassDeclaration(node));
const selected = new Set(['timeUnits','sizeUnits','formatSize','displayUnit','scaled','browse','applySnapshot','editorPointerDown','editorClick','isEditorBackdrop','closeEditor']);
const printer = ts.createPrinter();
const methods = component.members.filter(member => member.name && selected.has(member.name.getText(source)))
  .map(member => printer.printNode(ts.EmitHint.Unspecified, member, source)).join('\n');
const code = ts.transpileModule(`class Harness {${methods}\n draft:any; async act(action:any){await action();}}`, {
  compilerOptions: {target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}
}).outputText;
let selectedDirectory=null;
let calls=[];
const invoke=async command=>{
  calls.push(command);
  if(command==='pick_directory')return selectedDirectory;
  return [];
};
const Harness=new Function('invoke',code+'\nreturn Harness;')(invoke);
const signal=value=>{const getter=()=>value;getter.set=next=>{value=next;};return getter;};

test('unit values preserve exact saved values and reject invalid input',()=>{
  const h=new Harness();
  assert.deepEqual(h.displayUnit(1024**2,h.sizeUnits,1024**2),[1,1024**2]);
  assert.deepEqual(h.displayUnit(3600,h.timeUnits,60),[1,3600]);
  assert.deepEqual(h.displayUnit(90,h.timeUnits,60),[90,1]);
  assert.equal(h.scaled(1.5,1024**2),1572864);
  assert.equal(h.scaled(0.1,1024**2),104858);
  for(const value of [-1,Infinity,NaN])assert.throws(()=>h.scaled(value,60));
});

test('folder selection preserves SORT suffix and cancellation preserves input',async()=>{
  const h=new Harness();h.draft={source:'C:\\Source',destination:'C:\\Old\\{year}\\{month}',action:'SORT'};
  selectedDirectory='D:\\Sorted';await h.browse('destination');
  assert.equal(h.draft.destination,'D:\\Sorted\\{year}\\{month}');
  selectedDirectory=null;await h.browse('source');assert.equal(h.draft.source,'C:\\Source');
});

test('backdrop dismisses only an outside click and respects saving lock',()=>{
  const h=new Harness();let closes=0;
  const dialog={getBoundingClientRect:()=>({left:100,right:500,top:100,bottom:500}),close:()=>{closes++;}};
  h.editor={nativeElement:dialog};h.busy=signal(false);h.error=signal('');h.reset=()=>{};
  const inside={target:dialog,clientX:150,clientY:150};
  const outside={target:dialog,clientX:50,clientY:50};
  h.editorPointerDown(inside);h.editorClick(outside);assert.equal(closes,0);
  h.editorPointerDown(outside);h.editorClick(inside);assert.equal(closes,0);
  h.editorPointerDown(outside);h.editorClick(outside);assert.equal(closes,1);
  h.busy.set(true);h.editorPointerDown(outside);h.editorClick(outside);assert.equal(closes,1);
});

test('state events update the active run, rules, history and schedule without polling',()=>{
  calls=[];
  const h=new Harness();h.active=signal(null);h.rules=signal([]);h.history=signal([]);h.nextRuns=signal(new Map());
  h.applySnapshot({rules:[{id:1}],history:[{id:2}],next:[{rule_id:1,time:null,waiting:false,error:null}],active:{run_id:3,stopping:false}});
  assert.equal(h.active().run_id,3);assert.deepEqual(h.rules(),[{id:1}]);assert.deepEqual(h.history(),[{id:2}]);assert.equal(h.nextRuns().get(1).rule_id,1);
  h.applySnapshot({rules:[],history:[],next:[],active:null});assert.equal(h.active(),null);assert.deepEqual(h.rules(),[]);assert.deepEqual(h.history(),[]);
  assert.equal(h.nextRuns().size,0);assert.deepEqual(calls,[]);
});
