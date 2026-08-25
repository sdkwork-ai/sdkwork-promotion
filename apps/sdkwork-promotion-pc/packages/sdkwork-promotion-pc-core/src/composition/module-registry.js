// 业务模块注册表：聚合 PC 应用业务包的模块清单，供 shell 查询已装配能力
// 创建业务模块注册表，初始可为空，由 shell 装配业务包时注入清单
export function createPromotionPcModuleRegistry(initial = []) {
    const modules = new Map();
    for (const manifest of initial) {
        modules.set(manifest.id, manifest);
    }
    return {
        modules,
        register(manifest) {
            modules.set(manifest.id, manifest);
        },
        get(id) {
            return modules.get(id);
        },
        list() {
            return Array.from(modules.values());
        },
    };
}
//# sourceMappingURL=module-registry.js.map