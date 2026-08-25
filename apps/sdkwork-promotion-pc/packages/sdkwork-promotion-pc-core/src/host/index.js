// 宿主适配器契约：定义 PC 应用与原生宿主（浏览器/桌面/平板）之间的能力边界
// 桌面与平板宿主由 pc-desktop 包注入，浏览器环境降级为最小能力集
// 创建宿主上下文，未注入适配器时返回空上下文以保持浏览器降级可用
export function createPromotionPcHostContext(adapter = null) {
    return { adapter };
}
// 浏览器环境下的最小宿主适配器，仅暴露 clipboard 与 notifications 能力检测
export function createPromotionPcBrowserHostAdapter() {
    if (typeof window === "undefined") {
        return null;
    }
    return {
        name: "browser",
        surface: "browser",
        capabilities: {
            deepLinks: false,
            clipboard: typeof navigator?.clipboard !== "undefined",
            fileDialogs: false,
            updater: false,
            tray: false,
            notifications: typeof window.Notification !== "undefined",
        },
        async invoke() {
            throw new Error("Browser host does not expose native commands");
        },
    };
}
//# sourceMappingURL=index.js.map