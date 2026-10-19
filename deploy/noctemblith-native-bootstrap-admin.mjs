// Operator-only bootstrap for the reviewed native Release. No HTTP listener.
import { createRequire } from 'node:module';
import { generateKeyPairSync, randomBytes } from 'node:crypto';
const req = createRequire('/misskey/packages/backend/package.json');
const bcrypt = req('bcryptjs');
const { loadConfig } = await import('/misskey/packages/backend/built/config.js');
const { createPostgresDataSource, entities } = await import('/misskey/packages/backend/built/postgres.js');
const apply = process.argv.includes('--apply');
let db;
try {
 const c = loadConfig();
 if (process.env.NODE_ENV !== 'production' ||
     process.env.NOCTEMBLITH_MANAFIELD_SSO_ENABLED !== 'true' ||
     c.db.schema !== 'mf_noctemblith_state' ||
     c.url !== 'https://noctemblith-dev.manafield.studio' || c.id !== 'aid') {
  throw new Error('Unexpected managed configuration');
 }
 const targets = Object.fromEntries(['MiMeta','MiUser','MiUserProfile','MiUserKeypair','MiUsedUsername'].map(name => {
  const entity = entities.find(t => t.name === name);
  if (!entity) throw new Error('Entity contract changed');
  return [name, entity];
 }));
 let input, hash, keypair;
 if (apply) {
  let raw = '';
  for await (const part of process.stdin) raw += part;
  input = JSON.parse(raw);
  if (!/^[a-z0-9_]{1,20}$/.test(input.username) ||
      typeof input.password !== 'string' || input.password.length < 12 ||
      Buffer.byteLength(input.password, 'utf8') > 72) throw new Error('Invalid credentials');
  hash = await bcrypt.hash(input.password, 8);
  keypair = generateKeyPairSync('rsa', {
   modulusLength: 2048,
   publicKeyEncoding: {type:'spki',format:'pem'},
   privateKeyEncoding: {type:'pkcs8',format:'pem'}
  });
 }
 db = createPostgresDataSource(c);
 db.setOptions({ logging:false, cache:false, synchronize:false, dropSchema:false });
 await db.initialize();
 await db.transaction(async manager => {
  if (apply) {
   await manager.query('LOCK TABLE "mf_noctemblith_state"."meta", "mf_noctemblith_state"."user" IN EXCLUSIVE MODE');
  } else {
   await manager.query('SET TRANSACTION READ ONLY');
  }
  const metas = await manager.find(targets.MiMeta, {order:{id:'DESC'}});
  if (metas.length !== 1 || metas[0].rootUserId !== null)
   throw new Error('Bootstrap refused: missing metadata or administrator already exists');
  if (await manager.count(targets.MiUser) !== 0 ||
      await manager.count(targets.MiUsedUsername) !== 0)
   throw new Error('Bootstrap refused: instance is not empty');
  if (!apply) return;
  const id = (Date.now()-946684800000).toString(36).padStart(8,'0') +
             (randomBytes(2).readUInt16LE() % 1296).toString(36).padStart(2,'0');
  await manager.insert(targets.MiUser, {
   id,username:input.username,usernameLower:input.username,host:null,
   token:randomBytes(8).toString('hex')
  });
  await manager.insert(targets.MiUserProfile, {userId:id,autoAcceptFollowed:true,password:hash});
  await manager.insert(targets.MiUserKeypair, {
   userId:id,publicKey:keypair.publicKey,privateKey:keypair.privateKey
  });
  await manager.insert(targets.MiUsedUsername, {username:input.username,createdAt:new Date()});
  const updated = await manager.update(targets.MiMeta, metas[0].id, {rootUserId:id,disableRegistration:true});
  if (updated.affected !== 1) throw new Error('Root assignment failed');
  const root = await manager.findOneBy(targets.MiMeta,{id:metas[0].id});
  if (root?.rootUserId !== id) throw new Error('Root verification failed');
  const profile = await manager.findOneBy(targets.MiUserProfile,{userId:id});
  if (!profile?.password || !await bcrypt.compare(input.password, profile.password))
   throw new Error('Profile verification failed');
 });
 console.log(apply ? 'PASS: initial local administrator committed; public signup remains blocked' :
                      'PASS: managed native configuration; empty instance; no existing administrator');
} catch (error) {
 console.error('Stopped:', ['Unexpected managed configuration','Entity contract changed',
  'Invalid credentials','Bootstrap refused: missing metadata or administrator already exists',
  'Bootstrap refused: instance is not empty','Root assignment failed','Root verification failed',
  'Profile verification failed'].includes(error.message) ? error.message : 'Bootstrap failed; no credentials printed');
 process.exitCode = 1;
} finally {
 if (db?.isInitialized) await db.destroy();
}
