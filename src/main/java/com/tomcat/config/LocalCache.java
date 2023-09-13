package com.tomcat.config;

import cn.hutool.cache.CacheUtil;
import cn.hutool.cache.impl.TimedCache;
import cn.hutool.core.date.DateUnit;

/**
 * 描述：
 *
 */
public class LocalCache {
    /**
     * 缓存时长
     */
    public static final long TIMEOUT = 60 * 12 * DateUnit.MINUTE.getMillis();
    /**
     * 清理间隔
     */
    private static final long CLEAN_TIMEOUT = 60 * 12 * DateUnit.MINUTE.getMillis();

    /**
     * 缓存用户对AI的初始化上下文，uid---- 初始化AI的prompt
     *                             ｜
     *                              - 用户配置信息
     *                             ｜
     *                              - 课程表
     */
    public static final TimedCache<String, Object> CACHE_INIT_MSG = CacheUtil.newTimedCache(TIMEOUT);

    /**
     * @description 缓存用户与AI的场景对话聊天记录 chatId---聊天上下文
     * @param null:
     * @return null
     * @author tomcat
     * @date 2023/9/13 1:41 PM
     */
    public static final TimedCache<String, Object> CACHE_CHAT_MSG = CacheUtil.newTimedCache(TIMEOUT);
    public static final TimedCache<String, Object> CACHE_UID_CHATID = CacheUtil.newTimedCache(TIMEOUT);

    /**
     * @description 离线消息缓存
     * @param null:
     * @return null
     * @author tomcat
     * @date 2023/9/13 2:07 PM
     */
    public static final TimedCache<String, Object> CACHE_OFFLINE_MSG = CacheUtil.newTimedCache(TIMEOUT);

    static {
        //启动定时任务
        CACHE_INIT_MSG.schedulePrune(CLEAN_TIMEOUT);
        CACHE_OFFLINE_MSG.schedulePrune(CLEAN_TIMEOUT);
        CACHE_UID_CHATID.schedulePrune(CLEAN_TIMEOUT);
    }
}
