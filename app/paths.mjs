import path from 'node:path';
import { ServiceError } from './errors.mjs';

export function containedPath(root, pathname, platform = path) {
  const resolvedRoot = platform.resolve(root);
  const candidate = platform.resolve(resolvedRoot, `.${pathname}`);
  const relative = platform.relative(resolvedRoot, candidate);
  if (relative === '..' || relative.startsWith(`..${platform.sep}`) || platform.isAbsolute(relative)) {
    throw new ServiceError('PermissionDenied', 'Invalid resource path.', 403);
  }
  return candidate;
}
