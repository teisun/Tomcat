package com.tomcat.controller.response;

import lombok.Data;

import java.util.List;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: Topics
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/5 5:53 PM
 * @version: 1.0
 */
@Data
public class TopicsResp{

    private List<Topic> topics;


    @Data
    public static class Topic {
        String topic;
        List<String> objective;
    }
}
