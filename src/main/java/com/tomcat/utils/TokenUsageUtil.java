package com.tomcat.utils;

import com.unfbx.chatgpt.entity.common.Usage;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.utils
 * @className: TokenUsageUtil
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/18 7:10 PM
 * @version: 1.0
 */
public class TokenUsageUtil {

    public static Usage addUsage(Usage usage0, Usage usage1){
        usage0.setPromptTokens(usage0.getPromptTokens()+usage1.getPromptTokens());
        usage0.setCompletionTokens(usage0.getCompletionTokens()+usage1.getCompletionTokens());
        usage0.setTotalTokens(usage0.getTotalTokens() + usage1.getTotalTokens());
        return usage0;
    }
}
