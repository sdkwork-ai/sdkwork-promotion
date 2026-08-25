// 业务模块契约：定义 PC 应用业务包向 shell 注册的模块清单结构
// 业务包通过 routes.ts 贡献路由，通过模块清单声明自身能力与版本
// 构造单个业务模块清单的工厂函数，便于业务包在 shell 装配时声明自身
export function createPromotionPcModuleManifest(manifest) {
    return manifest;
}
//# sourceMappingURL=index.js.map