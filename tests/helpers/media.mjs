export async function drainRender(rendered) {
  const chunks = [];
  try {
    for await (const chunk of rendered.handle.createReadStream({ start: 0, autoClose: false })) chunks.push(chunk);
    return Buffer.concat(chunks);
  } finally {
    await rendered.handle.close();
  }
}
