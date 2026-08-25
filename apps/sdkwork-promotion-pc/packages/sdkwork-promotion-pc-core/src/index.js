// PC 应用核心运行时出口：runtimeIdentity、路由贡献契约与组合层再导出
// 对齐 AGENTS.md：domain=commerce，capability=promotion
// PC 应用运行时身份：对齐 AGENTS.md 的 domain=commerce / capability=promotion
export const sdkworkPromotionPcRuntimeIdentity = {
    appKey: "sdkwork-promotion-pc",
    architecture: "pc-react",
    domain: "commerce",
    capability: "promotion",
    runtimeFamily: "web",
    version: "0.1.0",
};
// 聚合业务包贡献的路由组，供 shell 装配路由表
export function createSdkworkPromotionPcRouteRegistry(...routeGroups) {
    return routeGroups.flat();
}
// 再导出 subpath 出口的类型与工厂，便于 shell 从主入口或子路径装配
export * from "./host/index.js";
export * from "./modules/index.js";
export * from "./sdk/index.js";
export * from "./session/index.js";
export * from "./composition/index.js";
//# sourceMappingURL=index.js.map