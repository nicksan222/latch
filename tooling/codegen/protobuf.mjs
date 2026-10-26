import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { fromBinary } from '@bufbuild/protobuf';
import { FileDescriptorSetSchema } from '@bufbuild/protobuf/wkt';

const protoRoot = new URL('../../packages/rpc/proto/', import.meta.url);
const protoPath = new URL('../../packages/rpc/proto/latch/v1/agent.proto', import.meta.url);
const generatedPackage = new URL('../../packages/rpc/ts/', import.meta.url);
const compatibilityPackage = new URL(
  '../../apps/desktop/src/_generated/contracts/',
  import.meta.url
);
const manifestTarget = new URL('../../packages/core/_generated/procedures.json', import.meta.url);
const desktopSdkTarget = new URL('../../apps/desktop/src/sdk/generated.ts', import.meta.url);
const plugin = new URL('../../node_modules/.bin/protoc-gen-es', import.meta.url);
const repository = localPath(new URL('../../', import.meta.url));

function localPath(url) {
  return fileURLToPath(url);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} failed (${result.status}): ${result.stderr || result.stdout}`);
  }
  return result.stdout;
}

export function compileProto({
  proto = localPath(protoPath),
  protoDirectory = localPath(protoRoot),
  pluginPath = localPath(plugin),
  protoc,
  outputDirectory
} = {}) {
  const temporary = mkdtempSync(join(tmpdir(), 'latch-proto-'));
  const tsOutput = outputDirectory ?? join(temporary, 'ts');
  try {
    mkdirSync(tsOutput, { recursive: true });
    const descriptorPath = join(temporary, 'descriptor.bin');
    const protocArgs = [
      `--proto_path=${protoDirectory}`,
      '--include_source_info',
      `--descriptor_set_out=${descriptorPath}`,
      `--plugin=protoc-gen-es=${pluginPath}`,
      `--es_out=${tsOutput}`,
      '--es_opt=target=ts',
      proto
    ];
    if (protoc ?? process.env.PROTOC) {
      run(protoc ?? process.env.PROTOC, protocArgs, { cwd: repository });
    } else {
      run(
        'cargo',
        ['run', '--locked', '-p', 'latch-rpc', '--example', 'protoc', '--', ...protocArgs],
        { cwd: repository }
      );
    }
    const descriptor = fromBinary(FileDescriptorSetSchema, readFileSync(descriptorPath));
    const file = descriptor.file.find((item) => item.name === 'latch/v1/agent.proto');
    if (!file) throw new Error('protoc descriptor is missing latch/v1/agent.proto');
    return { descriptor: file, tsOutput };
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function snakeToCamel(name) {
  return name.replace(/_([a-z0-9])/g, (_, char) => char.toUpperCase());
}

function fullName(typeName) {
  const parts = typeName?.replace(/^\./, '').split('.') ?? [];
  return parts.length > 3 ? parts.slice(2).join('.') : parts.at(-1);
}

function commentsByPath(descriptor) {
  const result = new Map();
  for (const location of descriptor.sourceCodeInfo?.location ?? []) {
    const comments = [location.leadingComments, location.trailingComments]
      .filter(Boolean)
      .join('\n');
    if (comments) result.set(location.path.join('.'), comments);
  }
  return result;
}

function enumLegacyValues(descriptor, comments) {
  const result = new Map();
  descriptor.enumType.forEach((enumeration, enumIndex) => {
    for (let valueIndex = 0; valueIndex < enumeration.value.length; valueIndex += 1) {
      const value = enumeration.value[valueIndex];
      const comment = comments.get(`5.${enumIndex}.2.${valueIndex}`) ?? '';
      const legacy = /legacy:\s*([^\s;]+)/.exec(comment)?.[1];
      if (legacy) result.set(value.name, legacy);
    }
  });
  return result;
}

const scalarTypes = new Map([
  [1, 'number'],
  [2, 'number'],
  [3, 'number'],
  [4, 'number'],
  [5, 'number'],
  [6, 'number'],
  [7, 'number'],
  [8, 'boolean'],
  [9, 'string'],
  [12, 'string'],
  [13, 'number'],
  [15, 'number'],
  [16, 'number'],
  [17, 'number'],
  [18, 'number']
]);

function fieldType(field, messages, comments) {
  const isRepeated = field.label === 3;
  let type = scalarTypes.get(field.type);
  if (field.type === 11) type = messageType(fullName(field.typeName), messages);
  if (field.type === 14) type = fullName(field.typeName);
  if (!type) throw new Error(`Unsupported protobuf field type ${field.type} for ${field.name}`);
  if (isRepeated) type = `Array<${type}>`;
  if (field.type === 11 && messages.get(fullName(field.typeName))?.options?.mapEntry) {
    const value = messages
      .get(fullName(field.typeName))
      .field.find((entry) => entry.name === 'value');
    type = `{ [key in string]?: ${scalarTypes.get(value.type) ?? 'unknown'} }`;
  }
  if (field.proto3Optional && !type.includes('| null')) type = `${type} | null`;
  const fieldComment = comments.get(`4.${field.messageIndex}.2.${field.fieldIndex}`) ?? '';
  if (field.type === 11 && /legacy_optional:\s*true/.test(fieldComment) && !type.includes('| null'))
    type = `${type} | null`;
  return type;
}

function isLegacyOmittable(field, comments) {
  return /legacy_omittable:\s*true/.test(
    comments.get(`4.${field.messageIndex}.2.${field.fieldIndex}`) ?? ''
  );
}

function messageType(name, messages) {
  if (name === 'Empty') return 'null';
  if (name === 'StringValue') return 'string';
  if (name === 'UInt32Value') return 'number';
  if (name === 'OptionalString') return 'string | null';
  if (name === 'OptionalEntryInput') return 'EntryInput | null';
  if (name === 'OptionalDomainRecipientKeyKind') return 'DomainRecipientKeyKind | null';
  if (name === 'OptionalParsedOtpauth') return 'ParsedOtpauth | null';
  if (name === 'OptionalStringList') return 'Array<string> | null';
  if (name === 'StringMap') return '{ [key in string]?: string }';
  if (name === 'DomainEnvVarList') return 'Array<EnvVar>';
  if (name === 'DomainStringList') return 'Array<string>';
  if (name === 'DomainDiffRowList') return 'Array<DomainDiffRow>';
  const message = messages.get(name);
  if (
    message?.field.length === 1 &&
    message.field[0].name === 'items' &&
    message.field[0].label === 3
  ) {
    const field = message.field[0];
    if (field.type === 11) return `Array<${fullName(field.typeName)}>`;
    return `Array<${scalarTypes.get(field.type) ?? 'unknown'}>`;
  }
  return name;
}

function messageImports(name, message, messages) {
  const helpers = new Set([
    'Empty',
    'StringValue',
    'UInt32Value',
    'OptionalString',
    'OptionalEntryInput',
    'OptionalDomainRecipientKeyKind',
    'OptionalParsedOtpauth',
    'OptionalStringList',
    'StringMap',
    'DomainEnvVarList',
    'DomainStringList',
    'DomainDiffRowList'
  ]);
  const imports = new Set();
  const visit = (field) => {
    if (field.type === 11) {
      const target = fullName(field.typeName);
      if (target === name) return;
      const targetMessage = messages.get(target);
      if (targetMessage?.options?.mapEntry) {
        const value = targetMessage.field.find((entry) => entry.name === 'value');
        if (value) visit(value);
        return;
      }
      if (helpers.has(target)) {
        for (const nested of targetMessage?.field ?? []) {
          if (nested.name === 'items' && ['StringMap', 'OptionalStringList'].includes(target))
            continue;
          visit(nested);
        }
        return;
      }
      if (targetMessage?.field.length === 1 && targetMessage.field[0].name === 'items') {
        visit(targetMessage.field[0]);
        return;
      }
      imports.add(target);
    } else if (field.type === 14) imports.add(fullName(field.typeName));
  };
  for (const field of message.field) visit(field);
  return [...imports].sort();
}

function typeForMessage(name, message, messages, comments) {
  const oneofs = message.oneofDecl
    .map((decl, index) => ({
      name: decl.name,
      fields: message.field.filter((field) => field.oneofIndex === index && !field.proto3Optional)
    }))
    .filter((oneof) => oneof.fields.length && !oneof.name.startsWith('_'));
  if (oneofs.length) {
    const dataKey =
      name === 'DomainResponse' ? 'output' : name === 'DomainRequest' ? 'input' : 'data';
    return oneofs[0].fields
      .map((field) => {
        const variant = fieldType(field, messages, comments);
        if (name === 'EntryPayload')
          return `{ "type": ${JSON.stringify(field.name)}, "data": ${variant} }`;
        return `{ "operation": ${JSON.stringify(field.name)}, "${dataKey}": ${variant} }`;
      })
      .join(' | ');
  }
  if (
    name === 'StringValue' ||
    name === 'UInt32Value' ||
    name === 'OptionalString' ||
    name === 'OptionalStringList' ||
    name === 'OptionalEntryInput' ||
    name === 'OptionalDomainRecipientKeyKind' ||
    name === 'OptionalParsedOtpauth' ||
    name === 'StringMap' ||
    name === 'DomainEnvVarList' ||
    name === 'DomainStringList' ||
    name === 'DomainDiffRowList' ||
    name === 'Empty'
  ) {
    return messageType(name, messages);
  }
  if (
    message.field.length === 1 &&
    message.field[0].name === 'items' &&
    message.field[0].label === 3
  ) {
    return messageType(name, messages);
  }
  const fields = message.field
    .map(
      (field) =>
        `${JSON.stringify(field.jsonName || snakeToCamel(field.name))}${isLegacyOmittable(field, comments) ? '?' : ''}: ${fieldType(field, messages, comments)}`
    )
    .join(', ');
  return `{ ${fields}${fields ? ', ' : ''}}`;
}

function sourceForContractTypes(descriptor) {
  const comments = commentsByPath(descriptor);
  const enums = new Map(
    descriptor.enumType.map((enumeration, enumIndex) => [
      enumeration.name,
      enumeration.value.slice(1).map((value, valueIndex) => ({
        ...value,
        legacy: enumLegacyValues(descriptor, comments).get(value.name) ?? value.name,
        location: `5.${enumIndex}.2.${valueIndex + 1}`
      }))
    ])
  );
  const messages = new Map(descriptor.messageType.map((message) => [message.name, message]));
  for (const message of descriptor.messageType) {
    for (const nested of message.nestedType) {
      messages.set(`${message.name}.${nested.name}`, nested);
    }
  }
  for (const [messageIndex, message] of descriptor.messageType.entries()) {
    for (const [fieldIndex, field] of message.field.entries()) {
      field.messageIndex = messageIndex;
      field.fieldIndex = fieldIndex;
    }
  }
  const outputs = [];
  for (const [name, message] of messages) {
    if (message.options?.mapEntry) continue;
    const imports = messageImports(name, message, messages);
    const header = `// Generated from the Protobuf descriptor. Do not edit.\n${imports.map((dependency) => `import type { ${dependency} } from './${dependency}';`).join('\n')}${imports.length ? '\n\n' : ''}`;
    outputs.push({
      name: `${name}.ts`,
      source: `${header}export type ${name} = ${typeForMessage(name, message, messages, comments)};\n`
    });
  }
  for (const [name, enumeration] of enums) {
    const values = enumeration.map((value) => JSON.stringify(value.legacy)).join(' | ') || 'never';
    outputs.push({
      name: `${name}.ts`,
      source: `// Generated from the Protobuf descriptor. Do not edit.\nexport type ${name} = ${values};\n`
    });
  }
  return outputs;
}

function resolveRpcType(typeName, messages) {
  const name = fullName(typeName);
  return messageType(name, messages);
}

function parseProcedureComments(raw, methodName) {
  const legacy = /legacy:\s*([^\s;]+)/.exec(raw ?? '')?.[1];
  const rust = /rust:\s*([^\s;]+)/.exec(raw ?? '')?.[1];
  const paramsMode = /params:\s*(none|empty_object|typed)/.exec(raw ?? '')?.[1];
  if (!legacy || !rust || !paramsMode) throw new Error(`Incomplete RPC metadata for ${methodName}`);
  return { legacy, rust, paramsMode };
}

export function manifestFromDescriptor(descriptor) {
  const comments = commentsByPath(descriptor);
  const messages = new Map(descriptor.messageType.map((message) => [message.name, message]));
  const service = descriptor.service.find((item) => item.name === 'LatchAgent');
  const methods =
    service?.method.filter((method) => !method.clientStreaming && !method.serverStreaming) ?? [];
  if (!service || methods.length < 61)
    throw new Error('LatchAgent is missing typed unary procedures');
  const procedures = methods.map((method) => {
    const index = service.method.indexOf(method);
    const metadata = parseProcedureComments(
      comments.get(`6.${descriptor.service.indexOf(service)}.2.${index}`),
      method.name
    );
    const requestType = fullName(method.inputType);
    const requestName = resolveRpcType(method.inputType, messages);
    const resultType = fullName(method.outputType);
    const response = messages.get(resultType);
    const resultName =
      response?.field?.length === 1 && response.field[0].name === 'items'
        ? fieldType(response.field[0], messages, comments)
        : resultType === 'Empty'
          ? 'null'
          : resultType === 'StringValue'
            ? 'string'
            : resultType === 'UInt32Value'
              ? 'number'
              : resultType === 'OptionalString'
                ? 'string | null'
                : resultType;
    const requestTypescript =
      metadata.paramsMode === 'none'
        ? 'void'
        : metadata.paramsMode === 'empty_object'
          ? 'Record<string, never>'
          : requestName;
    const resultTypescript = resultName;
    const primitiveOrHelper = new Set([
      'Empty',
      'StringValue',
      'UInt32Value',
      'OptionalString',
      'OptionalEntryInput',
      'OptionalDomainRecipientKeyKind',
      'OptionalParsedOtpauth',
      'OptionalStringList',
      'StringMap',
      'DomainEnvVarList',
      'DomainStringList',
      'DomainDiffRowList'
    ]);
    const requestImport =
      metadata.paramsMode === 'typed' &&
      messages.has(requestType) &&
      !primitiveOrHelper.has(requestType)
        ? requestType
        : null;
    const resultImport =
      response?.field?.length === 1 && response.field[0].name === 'items'
        ? response.field[0].type === 11
          ? fullName(response.field[0].typeName)
          : null
        : messages.has(resultType) && !primitiveOrHelper.has(resultType)
          ? resultType
          : null;
    return {
      id: metadata.rust.toUpperCase(),
      name: metadata.legacy,
      rust: metadata.rust,
      paramsMode: metadata.paramsMode,
      request: { typescript: requestTypescript, proto: requestType },
      result: { typescript: resultTypescript, proto: resultType },
      imports: [...new Set([requestImport, resultImport].filter(Boolean))].sort()
    };
  });
  return { version: 1, procedures };
}

export function generateProtoOutputs(descriptor) {
  const types = sourceForContractTypes(descriptor);
  const manifest = manifestFromDescriptor(descriptor);
  const imports = [
    ...new Set(manifest.procedures.flatMap((procedure) => procedure.imports))
  ].sort();
  const importLines = imports.map((name) => `import type { ${name} } from './${name}';`).join('\n');
  const methodMap = manifest.procedures
    .map(
      (procedure) =>
        `  ${JSON.stringify(procedure.name)}: { params: ${procedure.request.typescript}; result: ${procedure.result.typescript} };`
    )
    .join('\n');
  const methodNames = manifest.procedures
    .map((procedure) => JSON.stringify(procedure.name))
    .join(', ');
  types.push({
    name: 'rpc-methods.ts',
    source: `// Generated from the Protobuf descriptor. Do not edit.\n${importLines}${imports.length ? '\n' : ''}export type MethodMap = {\n${methodMap}\n};\n\nexport const METHODS = [${methodNames}] as const satisfies readonly (keyof MethodMap)[];\n`
  });
  return { types, manifest };
}

export function defaultProtoTargets() {
  return {
    packageTypes: localPath(generatedPackage),
    contracts: localPath(compatibilityPackage),
    desktopSdk: localPath(desktopSdkTarget),
    manifest: localPath(manifestTarget)
  };
}
