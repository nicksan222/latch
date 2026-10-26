// Desktop's JSON compatibility client is derived from Protobuf procedure metadata.
export function generateDesktopSdk(manifest) {
  const procedures = manifest?.procedures;
  if (manifest?.version !== 1 || !Array.isArray(procedures) || !procedures.length) {
    throw new Error('Missing or empty procedure manifest');
  }
  const ids = new Set();
  const names = new Set();
  for (const p of procedures) {
    if (
      !/^[A-Z][A-Z_0-9]*$/.test(p.id ?? '') ||
      !/^[a-z][a-zA-Z0-9]*(\.[a-z][a-zA-Z0-9]*)?$/.test(p.name ?? '') ||
      ids.has(p.id) ||
      names.has(p.name) ||
      !['none', 'empty_object', 'typed'].includes(p.paramsMode) ||
      !p.request?.typescript ||
      !p.result?.typescript ||
      !p.request?.proto ||
      !p.result?.proto ||
      !Array.isArray(p.imports)
    ) {
      throw new Error(`Invalid or duplicate procedure: ${p.id ?? '<missing>'}`);
    }
    ids.add(p.id);
    names.add(p.name);
  }
  const imports = [...new Set(procedures.flatMap((p) => p.imports))].sort();
  const groups = new Map();
  const calls = [];
  for (const p of procedures) {
    const parts = p.name.split('.');
    const path = parts.join('.');
    const typed = p.paramsMode === 'typed';
    const args = typed ? `params: ${p.request.typescript}` : '';
    const params = typed ? 'params' : p.paramsMode === 'none' ? 'null' : '{}';
    const fn = `(${args}): Promise<${p.result.typescript}> => invoke<${p.result.typescript}>('rpc', { method: '${p.name}', params: ${params} })`;
    if (parts.length === 1) {
      if (groups.has(parts[0])) throw new Error(`Conflicting procedure namespace: ${p.name}`);
      groups.set(parts[0], fn);
    } else {
      if (!groups.has(parts[0])) groups.set(parts[0], []);
      const group = groups.get(parts[0]);
      if (!Array.isArray(group)) throw new Error(`Conflicting procedure namespace: ${p.name}`);
      group.push(`      ${parts[1]}: ${fn},`);
    }
    calls.push(
      `      case '${p.name}': return procedures.${path}(${typed ? `params as ${p.request.typescript}` : ''});`
    );
  }
  const header = '// Generated from the Protobuf descriptor via procedures.json. Do not edit.\n';
  const desktop =
    header +
    `import { invoke as officialInvoke } from '@tauri-apps/api/core';\nimport type { ${imports.join(', ')} } from '../_generated/contracts';\n\nexport function createDesktopSdk(invoke: typeof officialInvoke = officialInvoke) {\n  const procedures = {\n` +
    [...groups]
      .map(([key, value]) =>
        Array.isArray(value)
          ? `    ${key}: {\n${value.join('\n')}\n    },`
          : `    ${key}: ${value},`
      )
      .join('\n') +
    `\n  };\n  return {\n    ...procedures,\n    async call(method: string, params: unknown): Promise<unknown> {\n      switch (method) {\n${calls.join('\n')}\n        default: throw new Error('Unknown RPC procedure');\n      }\n    },\n  };\n}\n`;
  return desktop;
}
