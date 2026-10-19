import { test } from 'node:test';
import assert from 'node:assert/strict';
import { assertBootstrapState } from './noctemblith-native-bootstrap-guard.mjs';

const empty = () => [[{rootUserId:null}], [], [], []];
function systemState(type = 'proxy') {
 return [
  [{rootUserId:null}],
  [{id:'sys',username:'system.'+type,usernameLower:'system.'+type,host:null,isBot:true,isLocked:true}],
  [{type,userId:'sys'}],
  [{username:'system.'+type}],
 ];
}
test('empty installation is accepted', () => {
 assert.deepEqual(assertBootstrapState(...empty()), []);
});
test('authentic system actors and their reservations are accepted', () => {
 for (const type of ['proxy','actor','relay']) {
  const state = systemState(type);
  const before = structuredClone(state);
  assert.deepEqual(assertBootstrapState(...state), [type]);
  assert.deepEqual(state, before);
 }
});
test('existing root is never overwritten', () => {
 const state = systemState();
 state[0][0].rootUserId = 'owner';
 assert.throws(() => assertBootstrapState(...state), /administrator already exists/);
});
test('ordinary user remains a hard refusal', () => {
 const state = systemState();
 state[1].push({id:'person',username:'eventide',usernameLower:'eventide',host:null});
 assert.throws(() => assertBootstrapState(...state), /non-system users/);
});
test('system name or bot flag alone does not bypass the guard', () => {
 const state = systemState();
 state[2] = [];
 assert.throws(() => assertBootstrapState(...state), /non-system users/);
});
test('dangling or forged system association is rejected', () => {
 for (const mutate of [
  s => s[2][0].userId = 'missing',
  s => s[2][0].type = 'admin',
  s => s[1][0].host = 'remote.example',
  s => s[1][0].isBot = false,
  s => s[1][0].isLocked = false,
  s => s[1][0].username = 'ordinary',
  s => s[2].push({...s[2][0]}),
 ]) {
  const state = systemState();
  mutate(state);
  assert.throws(() => assertBootstrapState(...state), /inconsistent system account/);
 }
});
test('deleted ordinary username reservation still prevents bootstrap', () => {
 const state = systemState();
 state[3].push({username:'former_user'});
 assert.throws(() => assertBootstrapState(...state), /username reservations/);
});
test('missing system reservation prevents bootstrap', () => {
 const state = systemState();
 state[3] = [];
 assert.throws(() => assertBootstrapState(...state), /username reservations/);
});
