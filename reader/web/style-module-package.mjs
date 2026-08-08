const STYLE_MODULE_KEYS = Object.freeze(["css", "enabled", "group", "id", "name"]);
const STYLE_MODULE_UTF8 = new TextEncoder();

export const STYLE_MODULE_LIMITS = Object.freeze({
  modules: 32,
  moduleBytes: 32768,
  combinedBytes: 65536,
  packageBytes: 6400000,
});

function ensure(condition, code = "invalid-style-module-package") {
  if (!condition) throw new Error(code);
}

function exact(value, expected) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const keys = Object.keys(value).sort();
  return keys.length === expected.length && keys.every((key, index) => key === expected[index]);
}

export function createStyleModulePackageCodec(validateStylesheet) {
  ensure(typeof validateStylesheet === "function");

  function validateModule(value, allowLegacyOversize = false) {
    ensure(exact(value, STYLE_MODULE_KEYS));
    const cssBytes = typeof value.css === "string" ? STYLE_MODULE_UTF8.encode(value.css).length : Infinity;
    ensure(
      typeof value.id === "string" &&
        /^[a-z0-9][a-z0-9-]{0,63}$/.test(value.id) &&
        typeof value.name === "string" &&
        value.name === value.name.trim() &&
        value.name.length > 0 &&
        value.name.length <= 64 &&
        !/[\u0000-\u001f\u007f]/u.test(value.name) &&
        typeof value.group === "string" &&
        value.group === value.group.trim() &&
        value.group.length <= 32 &&
        !/[\u0000-\u001f\u007f]/u.test(value.group) &&
        typeof value.enabled === "boolean" &&
        typeof value.css === "string" &&
        (cssBytes <= STYLE_MODULE_LIMITS.moduleBytes ||
          (allowLegacyOversize &&
            value.id === "legacy-user-css" &&
            value.css.length <= STYLE_MODULE_LIMITS.moduleBytes)),
    );
    return Object.freeze({ ...value });
  }

  function validateModules(values, allowLegacyOversize = false) {
    ensure(Array.isArray(values) && values.length <= STYLE_MODULE_LIMITS.modules);
    const ids = new Set();
    let total = 0;
    const modules = [];
    for (const [index, value] of values.entries()) {
      let module;
      try {
        module = validateModule(value, allowLegacyOversize);
        const cssBytes = STYLE_MODULE_UTF8.encode(module.css).length;
        if (cssBytes <= STYLE_MODULE_LIMITS.combinedBytes) validateStylesheet(module.css);
        else ensure(allowLegacyOversize && module.id === "legacy-user-css" && !module.enabled);
        ensure(!ids.has(module.id));
        const nextTotal = total + (module.enabled ? cssBytes : 0);
        ensure(nextTotal <= STYLE_MODULE_LIMITS.combinedBytes);
        ids.add(module.id);
        total = nextTotal;
      } catch (error) {
        const detail = error instanceof Error ? error : new Error("invalid-style-module-package");
        detail.moduleIndex = index;
        if (module?.id) detail.moduleId = module.id;
        if (typeof value?.name === "string" && value.name.trim()) {
          detail.moduleName = value.name.trim().slice(0, 64);
        }
        throw detail;
      }
      modules.push(module);
    }
    return Object.freeze(modules);
  }

  function parse(source) {
    ensure(
      typeof source === "string" &&
        source.length <= STYLE_MODULE_LIMITS.packageBytes &&
        STYLE_MODULE_UTF8.encode(source).length <= STYLE_MODULE_LIMITS.packageBytes,
    );
    const value = JSON.parse(source);
    ensure(exact(value, ["modules", "schema"]) && value.schema === 1);
    return validateModules(value.modules);
  }

  function stringify(modules) {
    return JSON.stringify({ schema: 1, modules: validateModules(modules, true) }, null, 2);
  }

  return Object.freeze({ parse, stringify, validateModules });
}
