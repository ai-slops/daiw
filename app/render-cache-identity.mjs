export function fileIdentity(file) {
	return {
		dev: file.dev,
		ino: file.ino,
		size: file.size,
		mtimeNs: file.mtimeNs,
		ctimeNs: file.ctimeNs,
	};
}

export function sameFileIdentity(left, right) {
	return (
		left.dev === right.dev &&
		left.ino === right.ino &&
		left.size === right.size &&
		left.mtimeNs === right.mtimeNs &&
		left.ctimeNs === right.ctimeNs
	);
}
