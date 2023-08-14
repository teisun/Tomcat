package com.tomcat.controller;

import com.tomcat.controller.requeset.AuthenticationRequest;
import com.tomcat.controller.requeset.ProfileResquest;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.utils.JsonUtil;
import com.tomcat.utils.JwtUtil;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestMethodOrder;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.security.access.method.P;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.test.web.servlet.ResultHandler;
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders;
import org.springframework.test.web.servlet.result.MockMvcResultMatchers;

import java.util.Optional;

@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
class UserProfileControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private JwtUtil jwtUtil;

    @Autowired
    private JsonUtil jsonUtil;

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;
    @Value("${jwt.tokenHead}")
    private String tokenHead;

    @Autowired
    UserRepository userRepository;

    @Test
    @Order(2)
    void getByUserId() throws Exception{
//        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        User user = userRepository.findByUsername("汤姆猫").orElseThrow(() -> new RuntimeException("用户不存在"));
        String token = jwtUtil.generateToken(user.getId(), user.getUsername());
        String userId = user.getId();

        mockMvc.perform(MockMvcRequestBuilders
                        .get("/profile/getProfile?userId="+userId)
                        .contentType(MediaType.APPLICATION_JSON_VALUE)
                        .header(tokenHeader,tokenHead+token)
                ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }

    @Test
    @Order(1)
    void updateProfile() throws Exception{
//        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        User user = userRepository.findByUsername("汤姆猫").orElseThrow(() -> new RuntimeException("用户不存在"));
        String token = jwtUtil.generateToken(user.getId(), user.getUsername());
//        ProfileResquest profileResquest = new ProfileResquest(2L,
//                "Chinese",
//                "high School",
//                "humor",
//                "English");
        ProfileResquest profileResquest = new ProfileResquest(user.getId(),
                "中文",
                "高中",
                "幽默",
                "English");
        String requestJson = jsonUtil.toJson(profileResquest);


        mockMvc.perform(MockMvcRequestBuilders
                        .put("/profile/update")
                        .content(requestJson.getBytes())
                        .accept(MediaType.APPLICATION_JSON)
                        .contentType(MediaType.APPLICATION_JSON_VALUE)
                        .header(tokenHeader,tokenHead+token)
                ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }
}