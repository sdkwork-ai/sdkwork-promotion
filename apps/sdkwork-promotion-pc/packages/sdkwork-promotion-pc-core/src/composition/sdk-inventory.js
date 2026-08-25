// SDK 客户端清单：声明 PC 应用依赖的生成式 SDK 客户端
// app 表面的 SDK 客户端由 pc-core 注入，backend-admin SDK 客户端由 pc-admin-core 边界负责
// 列出 PC 应用当前装配的 SDK 客户端清单
// 当前 PC 应用仅消费 app-api 表面的 promotion SDK 客户端，内容为空数组表示尚未装配
export function listPromotionPcSdkClients() {
    return [];
}
//# sourceMappingURL=sdk-inventory.js.map