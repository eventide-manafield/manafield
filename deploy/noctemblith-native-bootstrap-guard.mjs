// SPDX-License-Identifier: AGPL-3.0-only
// Accept only genuine internal accounts with matching system_account relations.
export function assertBootstrapState(metas, users, systems, used) {
 if (metas.length !== 1 || metas[0].rootUserId !== null) {
  throw new Error('Bootstrap refused: missing metadata or administrator already exists');
 }
 const types = new Set(['actor', 'relay', 'proxy']);
 const seenTypes = new Set();
 const seenIds = new Set();
 const names = new Set();
 for (const system of systems) {
  const user = users.find(u => u.id === system.userId);
  if (!types.has(system.type) || seenTypes.has(system.type) || seenIds.has(system.userId) ||
      !user || user.host !== null || user.username !== 'system.' + system.type ||
      user.usernameLower !== 'system.' + system.type || user.isBot !== true ||
      user.isLocked !== true) {
   throw new Error('Bootstrap refused: inconsistent system account records');
  }
  seenTypes.add(system.type);
  seenIds.add(system.userId);
  names.add(user.usernameLower);
 }
 if (users.length !== seenIds.size) {
  throw new Error('Bootstrap refused: non-system users already exist');
 }
 if (used.length !== names.size || used.some(u => !names.has(u.username)) ||
     new Set(used.map(u => u.username)).size !== names.size) {
  throw new Error('Bootstrap refused: non-system or inconsistent username reservations exist');
 }
 return [...seenTypes].sort();
}
