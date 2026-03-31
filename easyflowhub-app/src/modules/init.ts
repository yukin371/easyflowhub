/**
 * 模块系统初始化
 * 在应用启动时自动注册所有内置模块
 */
import { registerBuiltinModules } from './builtin';

// 执行注册
registerBuiltinModules();
