// SDK 客户端契约：定义 PC 应用消费的生成式 SDK 客户端类型与工厂
// 实际 SDK 客户端由 root src/bootstrap 在运行时构造并注入，core 仅声明契约
// 创建 SDK 客户端注册表，未注入时返回空注册表
export function createPromotionPcSdkClientRegistry(initial = []) {
    const clients = new Map();
    for (const entry of initial) {
        clients.set(entry.name, entry);
    }
    return {
        clients,
        register(entry) {
            clients.set(entry.name, entry);
        },
        get(name) {
            return clients.get(name);
        },
        list() {
            return Array.from(clients.values());
        },
    };
}
//# sourceMappingURL=index.js.map